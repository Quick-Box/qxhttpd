use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(FieldsWithValue)]
pub fn fields_with_value(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let struct_name = input.ident;

    let fields = if let syn::Data::Struct(syn::DataStruct {
                                              fields: syn::Fields::Named(ref fields),
                                              ..
                                          }) = input.data {
        fields
    } else {
        panic!("Only named struct fields are supported");
    };

    let checks = fields.named.iter().filter_map(|field| {
        let field_name = field.ident.as_ref()?;
        let field_ty = &field.ty;

        // Check if the field is of type Option
        if let syn::Type::Path(type_path) = field_ty {
            if type_path.path.segments.first()?.ident == "Option" {
                Some(quote! {
                    if self.#field_name.is_some() {
                        fields.push(stringify!(#field_name));
                    }
                })
            } else {
                None
            }
        } else {
            None
        }
    });

    let expanded = quote! {
        impl #struct_name {
            pub fn fields_with_value(&self) -> Vec<&'static str> {
                let mut fields = Vec::new();
                #(#checks)*
                fields
            }
        }
    };

    TokenStream::from(expanded)
}
