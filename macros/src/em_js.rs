use derive_syn_parse::Parse;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use std::{ffi::CString, fmt::Display, ops::Deref};
use syn::{parse_macro_input, spanned::Spanned, Attribute, Pat, Signature, Visibility};

// waiting on [https://github.com/rust-lang/rust/issues/85045]
#[allow(non_snake_case)]
pub fn EM_JS(items: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let EmJs {
        attrs,
        vis,
        sig,
        block,
    } = parse_macro_input!(items as EmJs);

    let mut pass_args = Vec::with_capacity(sig.inputs.len());
    let js_args = tri!(sig
        .inputs
        .iter()
        .map(|arg| match arg {
            syn::FnArg::Receiver(recv) => Err(syn::Error::new(
                recv.span(),
                "`self` is not allowed on JavaScript functions",
            )),
            syn::FnArg::Typed(arg) => match JsTypeable::from_type(&arg.ty) {
                Some(ty) => Ok((
                    match arg.pat.deref() {
                        Pat::Ident(i) => {
                            pass_args.push(&i.ident);
                            &i.ident
                        }
                        _ =>
                            return Err(syn::Error::new(
                                arg.pat.span(),
                                "Arguments must have an ident pattern",
                            )),
                    },
                    ty
                )),
                None => Err(syn::Error::new(
                    arg.span(),
                    "This argument cannot be translated into a JavaScript value",
                )),
            },
        })
        .collect::<Result<Vec<_>, _>>());

    let ident = &sig.ident;
    let fn_token = sig.fn_token;
    let inputs = &sig.inputs;
    let variadic = &sig.variadic;
    let output = &sig.output;

    let link_name = ident.to_string();
    let extern_ident = format_ident!("__extern_{ident}");
    let export_string_name = format_ident!("__em_js__{ident}");
    let ref_name = format_ident!("__em_js_ref_{ident}");

    let fn_str_args = string_join(js_args.iter().map(|(n, ty)| format!("{ty} {n}")), ",");
    let fn_str = match CString::new(format!(
        "({fn_str_args})<::>{{ {} }}",
        block.tokens.to_string()
    )) {
        Ok(bytes) => bytes.into_bytes_with_nul(),
        Err(e) => {
            return syn::Error::new(block.tokens.span(), e)
                .into_compile_error()
                .into()
        }
    };
    let fn_str_len = fn_str.len();

    let tokens = quote! {
        #[inline]
        #(#attrs)* #vis #sig {
            #[no_mangle]
            #[used]
            #[link_section = ".em_js"]
            static mut #export_string_name: [u8; #fn_str_len] = [#(#fn_str),*];

            extern "C" {
                #[link_name = #link_name]
                #fn_token #extern_ident (#inputs #variadic) #output;
            }

            #[no_mangle]
            #[used]
            static mut #ref_name: unsafe extern "C" #fn_token (#inputs #variadic) #output = #extern_ident;

            return #extern_ident(#(#pass_args),*)
        }
    };

    // panic!("{}", tokens.to_string());
    return tokens.into();
}

#[derive(Parse)]
struct EmJs {
    #[call(Attribute::parse_outer)]
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub sig: Signature,
    pub block: JsBlock,
}

#[derive(Parse)]
struct JsBlock {
    #[brace]
    pub brace_token: syn::token::Brace,
    #[inside(brace_token)]
    pub tokens: TokenStream,
}

enum JsTypeable {
    Bool,
    U8,
    U16,
    U32,
    U64,
    Usize,
    I8,
    I16,
    I32,
    I64,
    Isize,
    F32,
    F64,
    CChar,
    CShort,
    CInt,
    CLong,
    CLongLong,
    CUChar,
    CUShort,
    CUInt,
    CULong,
    CULongLong,
    ConstPtr(Box<Self>),
    MutPtr(Box<Self>),
}

impl JsTypeable {
    pub fn from_type(ty: &syn::Type) -> Option<JsTypeable> {
        return Some(match ty {
            syn::Type::Path(path) => match path.path.get_ident()?.to_string().as_str() {
                "bool" => Self::Bool,
                "u8" => Self::U8,
                "u16" => Self::U16,
                "u32" => Self::U32,
                "u64" => Self::U64,
                "usize" => Self::Usize,
                "i8" => Self::I8,
                "i16" => Self::I16,
                "i32" => Self::I32,
                "i64" => Self::I64,
                "isize" => Self::Isize,
                "c_char" => Self::CChar,
                "c_short" => Self::CShort,
                "c_int" => Self::CInt,
                "c_long" => Self::CLong,
                "c_longlong" => Self::CLongLong,
                "c_uchar" => Self::CUChar,
                "c_ushort" => Self::CUShort,
                "c_uint" => Self::CUInt,
                "c_ulong" => Self::CULong,
                "c_ulonglong" => Self::CULongLong,
                "f32" => Self::F32,
                "f64" => Self::F64,
                _ => return None,
            },

            syn::Type::Ptr(ptr) => {
                let pointee = JsTypeable::from_type(&ptr.elem)?;
                match (ptr.const_token.is_some(), ptr.mutability.is_some()) {
                    (true, false) => Self::ConstPtr(Box::new(pointee)),
                    (false, true) => Self::MutPtr(Box::new(pointee)),
                    _ => unreachable!(),
                }
            }

            syn::Type::Reference(refr) => {
                let pointee = JsTypeable::from_type(&refr.elem)?;
                match refr.mutability.is_some() {
                    false => Self::ConstPtr(Box::new(pointee)),
                    true => Self::MutPtr(Box::new(pointee)),
                }
            }

            _ => return None,
        });
    }
}

impl ToTokens for JsTypeable {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let stdlib = quote!(::em_bindgen::libstd);
        let primitive = quote!(#stdlib::primitive);
        let ffi = quote!(#stdlib::ffi);

        tokens.extend(match self {
            JsTypeable::Bool => quote!(#primitive::bool),
            JsTypeable::U8 => quote!(#primitive::u8),
            JsTypeable::U16 => quote!(#primitive::u16),
            JsTypeable::U32 => quote!(#primitive::u32),
            JsTypeable::U64 => quote!(#primitive::u64),
            JsTypeable::Usize => quote!(#primitive::usize),
            JsTypeable::I8 => quote!(#primitive::i8),
            JsTypeable::I16 => quote!(#primitive::i16),
            JsTypeable::I32 => quote!(#primitive::i32),
            JsTypeable::I64 => quote!(#primitive::i64),
            JsTypeable::Isize => quote!(#primitive::isize),
            JsTypeable::CChar => quote!(#ffi::c_char),
            JsTypeable::CShort => quote!(#ffi::c_short),
            JsTypeable::CInt => quote!(#ffi::c_int),
            JsTypeable::CLong => quote!(#ffi::c_long),
            JsTypeable::CLongLong => quote!(#ffi::c_longlong),
            JsTypeable::CUChar => quote!(#ffi::c_uchar),
            JsTypeable::CUShort => quote!(#ffi::c_ushort),
            JsTypeable::CUInt => quote!(#ffi::c_uint),
            JsTypeable::CULong => quote!(#ffi::c_ulong),
            JsTypeable::CULongLong => quote!(#ffi::c_ulonglong),
            JsTypeable::F32 => quote!(#primitive::f32),
            JsTypeable::F64 => quote!(#primitive::f64),
            JsTypeable::ConstPtr(pointee) => quote!(*const #pointee),
            JsTypeable::MutPtr(pointee) => quote!(*mut #pointee),
        })
    }
}

impl Display for JsTypeable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsTypeable::Bool => f.write_str("bool"),
            JsTypeable::U8 => f.write_str("uint8_t"),
            JsTypeable::U16 => f.write_str("uint16_t"),
            JsTypeable::U32 => f.write_str("uint32_t"),
            JsTypeable::U64 => f.write_str("uint64_t"),
            JsTypeable::Usize => f.write_str("uintptr_t"),
            JsTypeable::I8 => f.write_str("int8_t"),
            JsTypeable::I16 => f.write_str("int16_t"),
            JsTypeable::I32 => f.write_str("int32_t"),
            JsTypeable::I64 => f.write_str("int64_t"),
            JsTypeable::Isize => f.write_str("intptr_t"),
            JsTypeable::CChar => f.write_str("char"),
            JsTypeable::CShort => f.write_str("short"),
            JsTypeable::CInt => f.write_str("int"),
            JsTypeable::CLong => f.write_str("long"),
            JsTypeable::CLongLong => f.write_str("long long"),
            JsTypeable::CUChar => f.write_str("unsigned char"),
            JsTypeable::CUShort => f.write_str("unsigned short"),
            JsTypeable::CUInt => f.write_str("unsigned int"),
            JsTypeable::CULong => f.write_str("unsigned long"),
            JsTypeable::CULongLong => f.write_str("unsigned long long"),
            JsTypeable::F32 => f.write_str("float"),
            JsTypeable::F64 => f.write_str("double"),
            JsTypeable::ConstPtr(pointee) => f.write_fmt(format_args!("const {pointee}*")),
            JsTypeable::MutPtr(pointee) => f.write_fmt(format_args!("{pointee}*")),
        }
    }
}

fn string_join(vals: impl IntoIterator<Item: AsRef<str>>, delim: impl AsRef<str>) -> String {
    let delim = delim.as_ref();
    return vals.into_iter().fold(String::new(), |mut prev, curr| {
        if !prev.is_empty() {
            prev.push_str(delim);
        }
        prev.push_str(curr.as_ref());
        prev
    });
}
