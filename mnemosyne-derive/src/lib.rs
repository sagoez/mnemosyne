extern crate proc_macro;
extern crate proc_macro2;

mod internal;
use internal::{getter::get_inner_attribute, AttributeArgs, COMMAND_ATTRIBUTE, EVENT_ATTRIBUTE};
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Derive the `Command` trait for an enum. The enum must have a `#[command(state = "...", directive = "...")]`
/// attribute, where the value is the name of the state type and the directive is the name of the
/// event type.
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Debug, Clone, Serialize, Deserialize, Event)]
/// pub struct UserState {
///     count: u64,
/// }
///
/// #[derive(Debug, Clone, Serialize, Deserialize, Event)]
/// #[event(state = "UserState")]
/// pub enum UserEvent {
///  Incremented(Incremented),
///  Decremented(Decremented),
///  Reset(Reset),
/// }
///
/// #[derive(Debug, Clone, Serialize, Command, Deserialize)]
/// #[command(state = "UserState", directive = "UserEvent")]
/// #[serde(tag = "type")]
/// pub enum UserCommand {
///    Increment(Increment),
///    Decrement(Decrement),
///    Reset(Reset),
/// }
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// pub struct Increment;
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// pub struct Decrement;
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// pub struct Reset;
/// ```
#[proc_macro_derive(Command, attributes(command))] // TODO: Improve to accept SOLO enums and deeply nested enums
pub fn derive_command(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens as a DeriveInput
    let input = parse_macro_input!(input as DeriveInput);

    // Extract the enum identifier and its variants
    let enum_ident = input.ident.clone();
    let mut match_arms_validate = quote! {};
    let mut match_arms_directive = quote! {};
    let mut match_arms_entity_id = quote! {};

    if let syn::Data::Enum(data) = input.clone().data {
        for variant in data.variants {
            let variant_ident = variant.ident;
            match_arms_validate.extend(quote! {
                #enum_ident::#variant_ident(command) => command.validate(state),
            });
            match_arms_entity_id.extend(quote! {
                #enum_ident::#variant_ident(command) => command.entity_id(),
            });
            match_arms_directive.extend(quote! {
                #enum_ident::#variant_ident(command) => command.directive(state),
            });
        }
    } else {
        return syn::Error::new_spanned(input, "Command derive macro only works on enums")
            .to_compile_error()
            .into();
    }
    let (state, directive) = match get_inner_attribute(&input.attrs, COMMAND_ATTRIBUTE) {
        Ok(AttributeArgs {
            state: Some(state),
            directive: Some(directive),
        }) => (state, directive),
        Ok(_) => {
            return syn::Error::new_spanned(
                input,
                "Command derive macro requires a `state` and `directive` attribute",
            )
            .to_compile_error()
            .into()
        }
        Err(e) => return e.to_compile_error().into(),
    };

    let state_ident = syn::Ident::new(&state, proc_macro2::Span::call_site());
    let directive_ident = syn::Ident::new(&directive, proc_macro2::Span::call_site());

    let gen = quote! {

    impl mnemosyne::prelude::Command<#state_ident> for #enum_ident {
                type T = #directive_ident;

                fn validate(&self, state: &#state_ident) -> Result<mnemosyne::Unit, mnemosyne::domain::Error> {
                    match self {
                        #match_arms_validate
                    }
                }

                fn directive(&self, state: &#state_ident) -> Result<mnemosyne::prelude::NonEmptyVec<Box<#directive_ident>>, mnemosyne::domain::Error> {
                    match self {
                        #match_arms_directive
                    }
                }

                fn entity_id(&self) -> String {
                    match self {
                        #match_arms_entity_id
                    }
                }
            }
        };

    gen.into()
}

/// Derive the `Event` trait for an enum. The enum must have a `#[event(state = "...")]`
/// attribute, where the value is the name of the state type.
///
/// # Example
///
/// ```rust,ignore
///
/// #[derive(Debug, Clone, Serialize, Deserialize, Event)]
/// pub struct UserState {
///     count: u64,
/// }
///
/// #[derive(Debug, Clone, Serialize, Deserialize, Event)]
/// #[event(state = "UserState")]
/// pub enum UserEvent {
///   Incremented(Incremented),
///   Decremented(Decremented),
///   Reset(Reset),
/// }
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Incremented;
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Decremented;
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Reset;
/// ```
#[proc_macro_derive(Event, attributes(event))]
pub fn derive_event(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens as a DeriveInput
    let input = parse_macro_input!(input as DeriveInput);

    // Extract the enum identifier and its variants
    let enum_ident = input.ident.clone();
    let mut match_arms_apply = quote! {};
    let mut match_arms_effects = quote! {};

    if let syn::Data::Enum(ref data) = input.data {
        for variant in data.variants.iter() {
            let variant_ident = &variant.ident;
            match_arms_apply.extend(quote! {
                #enum_ident::#variant_ident(event) => event.apply(state),
            });
            match_arms_effects.extend(quote! {
                #enum_ident::#variant_ident(event) => event.effects(before, after),
            });
        }
    } else {
        return syn::Error::new_spanned(input, "Event derive macro only works on enums")
            .to_compile_error()
            .into();
    }

    let state = match get_inner_attribute(&input.attrs, EVENT_ATTRIBUTE) {
        Ok(AttributeArgs {
            state: Some(state), ..
        }) => state,
        Ok(_) => {
            return syn::Error::new_spanned(
                input,
                "Event derive macro requires a `state` attribute",
            )
            .to_compile_error()
            .into()
        }
        Err(e) => return e.to_compile_error().into(),
    };

    let state_ident = syn::Ident::new(&state, proc_macro2::Span::call_site());

    // Generate the trait implementation code
    let gen = quote! {
        impl Event<#state_ident> for #enum_ident {
            fn apply(&self, state: &#state_ident) -> Result<#state_ident, mnemosyne::domain::Error> {
                match self {
                    #match_arms_apply
                }
            }

            fn effects(&self, before: &#state_ident, after: &#state_ident) -> mnemosyne::Unit {
                match self {
                    #match_arms_effects
                }
            }
        }
    };

    gen.into()
}
