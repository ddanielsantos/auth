use crate::error::AppError;

pub fn parse_uuid(input: &impl AsRef<str>) -> Result<uuid::Uuid, AppError> {
    let uuid = uuid::Uuid::try_parse(input.as_ref())?;

    if uuid.get_version() != Some(uuid::Version::SortRand) {
        return Err(AppError::InvalidUUIDVersion);
    }

    Ok(uuid)
}

pub fn new_uuid() -> uuid::Uuid {
    uuid::Uuid::now_v7()
}