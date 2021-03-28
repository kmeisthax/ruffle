//! Storage for AS3 Vectors

use crate::avm2::activation::Activation;
use crate::avm2::class::Class;
use crate::avm2::names::{Namespace, QName};
use crate::avm2::object::{Object, TObject};
use crate::avm2::value::Value;
use crate::avm2::Error;
use gc_arena::{Collect, GcCell};

/// The vector storage portion of a vector object.
///
/// Vector values are restricted to a single type, decided at the time of the
/// construction of the vector's storage. The type is determined by the type
/// argument associated with the class of the vector. Vector holes are
/// evaluated to a default value based on the type of the vector.
///
/// A vector may also be configured to have a fixed size; when this is enabled,
/// attempts to modify the length fail.
#[derive(Collect, Debug, Clone)]
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
    value_proto: Object<'gc>,
}

impl<'gc> VectorStorage<'gc> {
    pub fn new(length: usize, is_fixed: bool, value_proto: Object<'gc>) -> Self {
        let mut storage = Vec::new();

        storage.resize(length, None);

        VectorStorage {
            storage,
            is_fixed,
            value_proto,
        }
    }

    pub fn set_is_fixed(&mut self, is_fixed: bool) {
        self.is_fixed = is_fixed;
    }

    pub fn length(&self) -> usize {
        self.storage.len()
    }

    pub fn resize(&mut self, new_length: usize) -> Result<(), Error> {
        if self.is_fixed {
            return Err("RangeError: Vector is fixed".into());
        }

        self.storage.resize(new_length, None);

        Ok(())
    }

    /// Get the default value for this vector.
    fn default(&self) -> Value<'gc> {
        if self.value_type().read().name() == &QName::new(Namespace::public(), "int") {
            Value::Integer(0)
        } else if self.value_type().read().name() == &QName::new(Namespace::public(), "uint") {
            Value::Unsigned(0)
        } else if self.value_type().read().name() == &QName::new(Namespace::public(), "Number") {
            Value::Number(0.0)
        } else {
            Value::Null
        }
    }

    pub fn value_proto(&self) -> Object<'gc> {
        self.value_proto
    }

    pub fn value_type(&self) -> GcCell<'gc, Class<'gc>> {
        self.value_proto.as_class().unwrap()
    }

    /// Coerce an incoming value into one compatible with our vector.
    ///
    /// You must call this before storing values in the vector with the type of
    /// the vector. The reason why this function is an associated type is
    /// because it can potentially execute user code and thus the containing
    /// vector object must not be locked.
    ///
    /// Values that cannot be coerced into the target type will be turned into
    /// `None` or yield an error as follows:
    ///
    ///  * The coercion fails
    ///  * The vector is of a non-coercible type, and the value is not an
    ///    instance or subclass instance of the vector's type
    pub fn coerce(
        from: Value<'gc>,
        to_type: Object<'gc>,
        activation: &mut Activation<'_, 'gc, '_>,
    ) -> Result<Option<Value<'gc>>, Error> {
        let to_class = to_type
            .as_class()
            .ok_or("TypeError: Cannot coerce to something that is not a type!")?;

        if to_class.read().name() == &QName::new(Namespace::public(), "int") {
            Ok(Some(from.coerce_to_i32(activation)?.into()))
        } else if to_class.read().name() == &QName::new(Namespace::public(), "uint") {
            Ok(Some(from.coerce_to_u32(activation)?.into()))
        } else if to_class.read().name() == &QName::new(Namespace::public(), "Number") {
            Ok(Some(from.coerce_to_number(activation)?.into()))
        } else if to_class.read().name() == &QName::new(Namespace::public(), "String") {
            Ok(Some(from.coerce_to_string(activation)?.into()))
        } else if to_class.read().name() == &QName::new(Namespace::public(), "Boolean") {
            Ok(Some(from.coerce_to_boolean().into()))
        } else if matches!(from, Value::Undefined) || matches!(from, Value::Null) {
            Ok(None)
        } else {
            let object_form = from.coerce_to_object(activation)?;
            if object_form.has_prototype_in_chain(to_type, true)? {
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
    pub fn get(&self, pos: usize) -> Result<Value<'gc>, Error> {
        self.storage
            .get(pos)
            .cloned()
            .map(|v| v.unwrap_or_else(|| self.default()))
            .ok_or_else(|| format!("RangeError: {} is outside the range of the vector", pos).into())
    }

    /// Store a value into the vector.
    ///
    /// This function does no coercion as calling it requires mutably borrowing
    /// the vector (and thus it is unwise to reenter the AVM2 runtime to coerce
    /// things). You must use the associated `coerce` fn before storing things
    /// in the vector.
    ///
    /// This function yields an error if the position is outside the length of
    /// the vector.
    pub fn set(&mut self, pos: usize, value: Option<Value<'gc>>) -> Result<(), Error> {
        self.storage
            .get_mut(pos)
            .map(|v| *v = value)
            .ok_or_else(|| format!("RangeError: {} is outside the range of the vector", pos).into())
    }

    /// Push a value to the end of the vector.
    ///
    /// This function does no coercion as calling it requires mutably borrowing
    /// the vector (and thus it is unwise to reenter the AVM2 runtime to coerce
    /// things). You must use the associated `coerce` fn before storing things
    /// in the vector.
    pub fn push(&mut self, value: Option<Value<'gc>>) {
        self.storage.push(value)
    }

    /// Iterate over vector values.
    pub fn iter<'a>(
        &'a self,
    ) -> impl DoubleEndedIterator<Item = Option<Value<'gc>>>
           + ExactSizeIterator<Item = Option<Value<'gc>>>
           + 'a {
        self.storage.iter().cloned()
    }
}
