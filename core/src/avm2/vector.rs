//! Storage for AS3 Vectors

use crate::avm2::activation::Activation;
use crate::avm2::names::{Multiname, Namespace, QName};
use crate::avm2::object::TObject;
use crate::avm2::value::Value;
use crate::avm2::Error;
use gc_arena::Collect;

/// The vector storage portion of a vector object.
///
/// Vector values are restricted to a single type, decided at the time of the
/// construction of the vector's storage. The type is determined by the type
/// argument associated with the class of the vector. Vector holes are
/// evaluated to a default value based on the
///
/// A vector may also be configured to have a fixed size; when this is enabled,
/// attempts to modify the length fail.
#[derive(Collect)]
#[collect(no_drop)]
pub struct VectorStorage<'gc> {
    /// The storage for vector values.
    ///
    /// While this is structured identically to `ArrayStorage`, the role of
    /// `None` values is significantly different. Instead of being treated as a
    /// hole to be resolved by reference to the prototype chain, vector holes
    /// are treated as default values. The actual default changes based on the
    /// type contained in the array.
    storage: Vec<Option<Value<'gc>>>,

    /// Whether or not the array length is fixed.
    is_fixed: bool,

    /// The allowed type of the contents of the vector.
    ///
    /// Vector typing is enforced by one of two ways: either by generating
    /// exceptions on values that are not of the given type, or by coercing
    /// incorrectly typed values to the given type if possible. Values that do
    /// not coerce would then be treated as vector holes and retrieved as a
    /// default value.
    value_type: Multiname<'gc>,
}

impl<'gc> VectorStorage<'gc> {
    fn new(length: usize, is_fixed: bool, value_type: Multiname<'gc>) -> Self {
        let mut storage = Vec::new();

        storage.resize(length, None);

        VectorStorage {
            storage,
            is_fixed,
            value_type,
        }
    }

    fn resize(&mut self, new_length: usize) -> Result<(), Error> {
        if self.is_fixed {
            return Err("RangeError: Vector is fixed".into());
        }

        self.storage.resize(new_length, None);

        Ok(())
    }

    /// Get the default value for this vector.
    fn default(&self) -> Value<'gc> {
        if self
            .value_type
            .is_satisfied_by_qname(&QName::new(Namespace::public(), "int"))
        {
            Value::Integer(0)
        } else if self
            .value_type
            .is_satisfied_by_qname(&QName::new(Namespace::public(), "uint"))
        {
            Value::Unsigned(0)
        } else if self
            .value_type
            .is_satisfied_by_qname(&QName::new(Namespace::public(), "Number"))
        {
            Value::Number(0.0)
        } else {
            Value::Null
        }
    }

    /// Coerce an incoming value into one compatible with our vector.
    ///
    /// Values that cannot be coerced into the target type will be turned into
    /// `None`.
    fn coerce(
        &self,
        from: Value<'gc>,
        activation: &mut Activation<'_, 'gc, '_>,
    ) -> Result<Option<Value<'gc>>, Error> {
        if self
            .value_type
            .is_satisfied_by_qname(&QName::new(Namespace::public(), "int"))
        {
            Ok(Some(from.coerce_to_i32(activation)?.into()))
        } else if self
            .value_type
            .is_satisfied_by_qname(&QName::new(Namespace::public(), "uint"))
        {
            Ok(Some(from.coerce_to_u32(activation)?.into()))
        } else if self
            .value_type
            .is_satisfied_by_qname(&QName::new(Namespace::public(), "Number"))
        {
            Ok(Some(from.coerce_to_number(activation)?.into()))
        } else if self
            .value_type
            .is_satisfied_by_qname(&QName::new(Namespace::public(), "String"))
        {
            Ok(Some(from.coerce_to_string(activation)?.into()))
        } else if self
            .value_type
            .is_satisfied_by_qname(&QName::new(Namespace::public(), "Boolean"))
        {
            Ok(Some(from.coerce_to_boolean().into()))
        } else if matches!(from, Value::Undefined) || matches!(from, Value::Null) {
            Ok(None)
        } else {
            let object_form = from.coerce_to_object(activation)?;
            if object_form.is_of_type(&self.value_type) {
                return Ok(Some(from));
            }

            Err(format!(
                "TypeError: cannot coerce object of type {}",
                object_form
                    .as_class()
                    .map(|c| c.read().name().local_name().to_string())
                    .unwrap_or_else(|| "".to_string())
            )
            .into())
        }
    }

    /// Retrieve a value from the vector.
    ///
    /// If the value is `None`, the type default value will be substituted.
    fn get(&self, pos: usize) -> Result<Value<'gc>, Error> {
        self.storage
            .get(pos)
            .cloned()
            .map(|v| v.unwrap_or_else(|| self.default()))
            .ok_or_else(|| format!("RangeError: {} is outside the range of the vector", pos).into())
    }

    /// Store a value into the vector.
    ///
    /// If the value is not of the vector's type, then the value will be
    /// coerced to fit as per `coerce`. This function yields an error if:
    ///
    ///  * The coercion fails
    ///  * The vector is of a non-coercible type, and the value is not an
    ///    instance or subclass instance of the vector's type
    ///  * The position is outside the length of the vector
    fn set(
        &mut self,
        pos: usize,
        value: Value<'gc>,
        activation: &mut Activation<'_, 'gc, '_>,
    ) -> Result<(), Error> {
        let coerced_value = self.coerce(value, activation)?;
        self.storage
            .get_mut(pos)
            .map(|v| *v = coerced_value)
            .ok_or_else(|| format!("RangeError: {} is outside the range of the vector", pos).into())
    }
}
