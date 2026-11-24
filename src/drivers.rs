use std::borrow::Cow;

use elemental::driver::driver::DriverDescriptor;

use crate::i18n;

/// Returns a localized driver name, falling back to the upstream display name.
pub fn display_name(driver: DriverDescriptor) -> Cow<'static, str> {
    let key = format!("catalog.driver.{}", driver.id);
    i18n::try_translate(&key).unwrap_or(Cow::Borrowed(driver.name))
}
