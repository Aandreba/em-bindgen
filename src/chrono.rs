use self::sys::{OffsetFromLocalDateTime, OffsetFromUtcDateTime};
use chrono::{
    DateTime, Datelike, FixedOffset, MappedLocalTime, NaiveTime, TimeZone, Timelike, Utc,
};

#[derive(Debug, Clone)]
pub struct Local;

impl Local {
    /// Returns a `DateTime<Local>` which corresponds to the current date, time and offset from
    /// UTC.
    ///
    /// See also the similar [`Utc::now()`] which returns `DateTime<Utc>`, i.e. without the local
    /// offset.
    ///
    /// # Example
    ///
    /// ```
    /// # #![allow(unused_variables)]
    /// # use chrono::{DateTime, FixedOffset, Local};
    /// // Current local time
    /// let now = Local::now();
    ///
    /// // Current local date
    /// let today = now.date_naive();
    ///
    /// // Current local time, converted to `DateTime<FixedOffset>`
    /// let now_fixed_offset = Local::now().fixed_offset();
    /// // or
    /// let now_fixed_offset: DateTime<FixedOffset> = Local::now().into();
    ///
    /// // Current time in some timezone (let's use +05:00)
    /// // Note that it is usually more efficient to use `Utc::now` for this use case.
    /// let offset = FixedOffset::east_opt(5 * 60 * 60).unwrap();
    /// let now_with_offset = Local::now().with_timezone(&offset);
    /// ```
    pub fn now() -> DateTime<Local> {
        Utc::now().with_timezone(&Local)
    }
}

impl TimeZone for Local {
    type Offset = FixedOffset;

    fn from_offset(_offset: &Self::Offset) -> Self {
        Local
    }

    #[allow(deprecated)]
    fn offset_from_local_date(
        &self,
        local: &chrono::prelude::NaiveDate,
    ) -> chrono::MappedLocalTime<Self::Offset> {
        // Get the offset at local midnight.
        self.offset_from_local_datetime(&local.and_time(NaiveTime::MIN))
    }

    fn offset_from_local_datetime(
        &self,
        local: &chrono::prelude::NaiveDateTime,
    ) -> chrono::MappedLocalTime<Self::Offset> {
        let mut year = local.year();
        if year < 100 {
            // Values for years from `0` to `99` map to the years `1900` to `1999`.
            // Shift the value by a multiple of 400 years until it is `>= 100`.
            let shift_cycles = (year - 100).div_euclid(400);
            year -= shift_cycles * 400;
        }

        let offset = unsafe {
            OffsetFromLocalDateTime(
                year,
                local.month0(),
                local.day(),
                local.hour(),
                local.minute(),
                local.second(),
            )
        };

        // We always get a result, even if this time does not exist or is ambiguous.
        MappedLocalTime::Single(FixedOffset::west_opt(offset * 60).unwrap())
    }

    #[allow(deprecated)]
    fn offset_from_utc_date(&self, utc: &chrono::prelude::NaiveDate) -> Self::Offset {
        // Get the offset at midnight.
        self.offset_from_utc_datetime(&utc.and_time(NaiveTime::MIN))
    }

    fn offset_from_utc_datetime(&self, utc: &chrono::prelude::NaiveDateTime) -> Self::Offset {
        let offset = unsafe { OffsetFromUtcDateTime(utc.and_utc().timestamp_millis() as f64) };
        MappedLocalTime::Single(FixedOffset::west_opt(offset * 60).unwrap()).unwrap()
    }
}

pub mod sys {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]

    include!(concat!(env!("OUT_DIR"), "/chrono.rs"));
}
