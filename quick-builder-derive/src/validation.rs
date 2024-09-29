//! this module helps us pass the #[validate(my_function)] and e.g.
//! #[validate(|arg| arg.len() == 3]. Valid expressions are closures with
//! one argument returning a bool, or paths that must point to a function
//! with one argument returning a bool. The argument must be of type
//! `&Foo` where `Foo` is the structure for which we created the builder.

use syn::{
    parse::{Parse, ParseBuffer},
    AttrStyle, Attribute, Expr, ExprClosure, Field, Meta, Path,
};

pub const VALIDATE_ATTR: &str = "validate";

use crate::error::CompileError;

pub struct ValidateAttribute {
    /// the expression in brackets in the validation attribute
    expression: ValidationExpression,
}

impl ValidateAttribute {
    /// try parsing the validate attribute from a list of attributes of a field.
    /// A field may have ZERO or ONE validate attributes, hence the Option<...>.
    /// The option has None value if the field does not have a validate attribute,
    /// otherwise it is Some(...). If an error occurs during parsing, or if more than
    /// one validate attribute is present, returns an error.
    pub fn try_from_attributes(attributes: &[Attribute]) -> Result<Option<Self>, CompileError> {
        // helper predicate that helps us find the validate attribute
        let is_validate_attribute = |attr: &Attribute| match attr.meta {
            Meta::Path(_) => false,
            Meta::List(ref list) => {
                list.path.segments.len() == 1 && list.path.segments[0].ident == VALIDATE_ATTR
            }
            Meta::NameValue(_) => false,
        };

        // get the zero or one validate attributes
        // return an error if one was encountered, also return an error when more
        // than one attribute exists.
        let result: Result<Option<&Attribute>, CompileError> =
            attributes.iter().fold(Ok(None), |init, curr| match init {
                Ok(None) => {
                    if is_validate_attribute(curr) {
                        Ok(Some(curr))
                    } else {
                        Ok(None)
                    }
                }
                Ok(Some(previous)) => {
                    if is_validate_attribute(curr) {
                        Err(CompileError::new_spanned(
                            curr,
                            "only one attribute of this kind allowed per item",
                        ))
                    } else {
                        Ok(Some(previous))
                    }
                }
                Err(err) => Err(err),
            });
        let maybe_validate_attr = result?;
        let Some(validate_attr) = maybe_validate_attr else {
            return Ok(None);
        };

        let expression = ValidationExpression::try_from(&validate_attr.meta)?;

        Ok(Some(Self { expression }))
    }
}

/// the expression for validation
enum ValidationExpression {
    /// a closure is defined
    /// (there are some aspects that we can verify, but not all)
    Closure(ExprClosure),
    /// a path to a function is given
    /// (there's nothing more about this we can verify at macro expansion time)
    Path(Path),
}

impl TryFrom<&Meta> for ValidationExpression {
    type Error = CompileError;
    fn try_from(meta: &Meta) -> Result<Self, CompileError> {
        match meta {
            Meta::Path(path) => Ok(Self::Path(path.clone())),
            Meta::List(list) => {
                // first try parsing this as a path
                if let Some(path) = syn::parse::<Path>(list.tokens.clone().into()).ok() {
                    return Ok(Self::Path(path));
                }
                // otherwise this must be a closure
                if let Some(closure) = syn::parse::<ExprClosure>(list.tokens.clone().into()).ok() {
                    // we can do some error checks for better error messages.
                    // We have no actual type information but we can make sure that
                    // the closure is a single-argument closure that is not async
                    if closure.asyncness.is_some() {
                        Err(CompileError::new_spanned(
                            &closure.asyncness,
                            "async in validation closure not allowed",
                        ))
                    } else if closure.capture.is_some() {
                        Err(CompileError::new_spanned(
                            &closure.capture,
                            "move capture in validation closure not allowed",
                        ))
                    } else if closure.inputs.len() != 1 {
                        Err(CompileError::new_spanned(
                            &closure.inputs,
                            "validation closure must have exactly one argument",
                        ))
                    } else {
                        Ok(Self::Closure(closure))
                    }
                } else {
                    Err(CompileError::new_spanned(
                        meta,
                        "validation argument must be a function or a single argument closure",
                    ))
                }
            }
            Meta::NameValue(value) => Err(CompileError::new_spanned(
                value,
                "QuickBuilder: Attribute arguments must either be a function name or a closure.",
            )),
        }
    }
}
