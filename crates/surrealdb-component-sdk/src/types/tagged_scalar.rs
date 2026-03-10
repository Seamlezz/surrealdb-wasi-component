use serde::Serializer;
use serde::ser::SerializeMap;

pub(crate) fn serialize_tagged_scalar<S>(
    serializer: S,
    tag: &str,
    value: &str,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut map = serializer.serialize_map(Some(1))?;
    map.serialize_entry(tag, value)?;
    map.end()
}
