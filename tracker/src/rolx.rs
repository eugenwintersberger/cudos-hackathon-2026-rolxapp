//! Activities and work records of the Rolx server for the work tracking.
//!
//! The functions block while they talk to the server: call them from a thread
//! of their own, not from the UI thread.

use std::{collections::HashSet, error, fmt};

use chrono::NaiveDate;
use device_flow::AppConfig;
use tracing::warn;
use uuid::Uuid;

use crate::login::MemorySessionStore;

type Record = api::types::RolXServerRecordsWebApiResourceRecord;
type RecordEntry = api::types::RolXServerRecordsWebApiResourceRecordEntry;
type ServerActivity = api::types::RolXServerProjectsWebApiResourceActivity;

/// An activity the user can book time on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    pub id: i32,
    /// `fullNumber` as reported by the server; stored in the record entry.
    pub number: Option<String>,
    /// Text for the selection list.
    pub label: String,
    /// First day the activity is open (`startDate`), if bounded.
    pub starts: Option<NaiveDate>,
    /// Last day the activity is open (`endDate`), if bounded.
    pub ends: Option<NaiveDate>,
    /// Whether the user marked the activity as a favourite.
    pub favourite: bool,
}

impl Activity {
    /// Whether the activity can be booked on `date`.
    pub fn is_open_on(&self, date: NaiveDate) -> bool {
        let starts = self.starts.is_none_or(|start| start <= date);
        let ends = self.ends.is_none_or(|end| date <= end);
        starts && ends
    }
}

/// The work record of one day.
#[derive(Debug, Clone)]
pub struct Day {
    pub date: NaiveDate,
    /// `None` when the server has no record for that day yet.
    record: Option<Record>,
}

impl Day {
    /// Seconds recorded on that day so far.
    pub fn recorded_seconds(&self) -> i64 {
        self.record
            .as_ref()
            .and_then(|record| record.entries.as_ref())
            .map(|entries| entries.iter().filter_map(|entry| entry.duration).sum())
            .unwrap_or(0)
    }

    /// Whether the server knows a record for that day already.
    pub fn has_record(&self) -> bool {
        self.record.is_some()
    }
}

#[derive(Debug)]
pub enum Error {
    /// No stored session, or Rolx rejected it: sign in again.
    Session(device_flow::Error),
    /// Rolx rejected the request (e.g. HTTP 401).
    Unauthorized,
    Api(String),
    InvalidUserId,
    Runtime(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Session(e) => write!(f, "not signed in: {e}"),
            Self::Unauthorized => write!(f, "the server rejected the session, sign in again"),
            Self::Api(e) => write!(f, "the server request failed: {e}"),
            Self::InvalidUserId => write!(f, "the session contains no valid user id"),
            Self::Runtime(e) => write!(f, "cannot start the network thread: {e}"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Session(e) => Some(e),
            _ => None,
        }
    }
}

/// A date in the format the Rolx API expects (`YYYY-mm-dd`).
pub fn date_text(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// The day range that contains exactly `date`.
///
/// Rolx takes the range as half-open (`begin <= date < end`); the server
/// itself uses `date` and `date + 1 day` for a single day.
pub fn day_range(date: NaiveDate) -> (String, String) {
    let end = date.succ_opt().unwrap_or(date);
    (date_text(date), date_text(end))
}

/// Label of an activity in the selection list.
pub fn activity_label(number: Option<&str>, name: Option<&str>, id: i32) -> String {
    let number = number.map(str::trim).unwrap_or_default();
    let name = name.map(str::trim).unwrap_or_default();
    match (number.is_empty(), name.is_empty()) {
        (false, false) => format!("{number} {name}"),
        (false, true) => number.to_owned(),
        (true, false) => name.to_owned(),
        (true, true) => format!("#{id}"),
    }
}

/// The record entry for `activity`.
///
/// Rolx counts seconds: `duration` is the length and `begin` the seconds since
/// midnight. `pause` stays empty; Rolx requires it to be `None` for entries
/// with a begin time.
pub fn record_entry(activity: &Activity, duration_seconds: i64, begin_seconds: i64) -> RecordEntry {
    RecordEntry {
        activity_id: Some(activity.id),
        full_activity_number: activity.number.clone(),
        duration: Some(duration_seconds),
        begin: Some(begin_seconds as i32),
        pause: None,
        comment: None,
    }
}

/// The record of a day the server does not know yet.
///
/// Only the fields the server needs are set, so that it applies its own
/// defaults for the day type and the nominal work time.
pub fn new_record(user_id: Uuid, date: NaiveDate, entries: Vec<RecordEntry>) -> Record {
    Record {
        date: Some(date_text(date)),
        user_id: Some(user_id),
        entries: Some(entries),
        ..Default::default()
    }
}

fn activity_from(server: &ServerActivity) -> Activity {
    let name = server.full_name.as_deref().or(server.name.as_deref());
    let id = server.id.unwrap_or_default();
    Activity {
        id,
        number: server.full_number.clone(),
        label: activity_label(server.full_number.as_deref(), name, id),
        starts: iso_day(server.start_date.as_deref()),
        ends: iso_day(server.end_date.as_deref()),
        favourite: false,
    }
}

/// The day of an activity bound; a bound that cannot be read counts as none.
fn iso_day(text: Option<&str>) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text?.trim(), "%Y-%m-%d").ok()
}

/// The activities to choose from: the relevant ones plus the user's
/// favourites, sorted by name.
///
/// The server returns the favourites unfiltered, so a favourite that is not
/// open on `date` is dropped (it could not be booked). Every activity appears
/// once, even when it is both relevant and a favourite.
pub fn merge(
    suitable: &[ServerActivity],
    favourites: &[ServerActivity],
    date: NaiveDate,
) -> Vec<Activity> {
    let favourite_ids: HashSet<i32> = favourites.iter().filter_map(|activity| activity.id).collect();
    let mut merged: Vec<Activity> = Vec::with_capacity(suitable.len() + favourites.len());
    let mut seen: HashSet<i32> = HashSet::new();

    for server in suitable {
        let mut activity = activity_from(server);
        activity.favourite = favourite_ids.contains(&activity.id);
        if seen.insert(activity.id) {
            merged.push(activity);
        }
    }
    for server in favourites {
        let mut activity = activity_from(server);
        activity.favourite = true;
        if activity.is_open_on(date) && seen.insert(activity.id) {
            merged.push(activity);
        }
    }

    merged.sort_by(|a, b| {
        a.label
            .to_lowercase()
            .cmp(&b.label.to_lowercase())
            .then(a.id.cmp(&b.id))
    });
    merged
}

/// A runtime for the blocking calls; the network stack needs `enable_all`.
fn runtime() -> Result<tokio::runtime::Runtime, Error> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| Error::Runtime(e.to_string()))
}

/// Turn a client error into ours; the status is the only part we look at and
/// the bearer token never appears in the message.
fn api_error(e: api::Error<()>) -> Error {
    match e.status() {
        Some(status) if status.as_u16() == 401 => Error::Unauthorized,
        Some(status) => Error::Api(format!(
            "HTTP {} {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or_default()
        )),
        None => Error::Api(e.to_string()),
    }
}

/// Whether a status the generated client did not expect still means success.
fn accepted_status(code: Option<u16>) -> bool {
    matches!(code, Some(code) if (200..300).contains(&code))
}

/// Rolx answers the record update with `204 No Content`, but the OpenAPI spec
/// only declares `200`, so the generated client reports that answer as an
/// unexpected response. Any success status means the record is stored.
fn stored(result: Result<api::ResponseValue<()>, api::Error<()>>) -> Result<(), Error> {
    match result {
        Ok(_) => Ok(()),
        Err(e) => match e.status().map(|status| status.as_u16()) {
            code if accepted_status(code) => Ok(()),
            _ => Err(api_error(e)),
        },
    }
}

/// A Rolx client for the signed-in user.
pub struct Rolx {
    client: api::Client,
    user_id: Uuid,
}

impl Rolx {
    /// Connect with the stored session, extending it first if it is about to
    /// expire. Blocks.
    pub fn connect(config: &AppConfig, store: &MemorySessionStore) -> Result<Self, Error> {
        let runtime = runtime()?;
        let session = runtime
            .block_on(async {
                let mut session = device_flow::whoami(store)?;
                if session.is_near_expiry() {
                    session = device_flow::extend_session(config, store).await?;
                }
                Ok::<_, device_flow::Error>(session)
            })
            .map_err(Error::Session)?;
        let user_id = Uuid::parse_str(session.user_id.trim()).map_err(|_| Error::InvalidUserId)?;
        let client = api::Client::create(&config.server_url, session.bearer_token.as_str())
            .map_err(|e| Error::Api(e.to_string()))?;
        Ok(Self { client, user_id })
    }

    /// The activities to choose from on `date`: the ones that fit the user and
    /// the day, plus the user's favourites, sorted by name. Blocks.
    pub fn activities(&self, date: NaiveDate) -> Result<Vec<Activity>, Error> {
        let runtime = runtime()?;
        let date_text = date_text(date);
        let suitable = runtime
            .block_on(
                self.client
                    .get_activity_suitable_user_id_date(&self.user_id, &date_text),
            )
            .map_err(api_error)?
            .into_inner();
        // The favourites are a bonus: without them the list still works.
        let favourites = match runtime.block_on(self.client.get_activity_favourite()) {
            Ok(list) => list.into_inner(),
            Err(e) => {
                warn!("cannot fetch the favourite activities: {e}");
                Vec::new()
            }
        };
        Ok(merge(&suitable, &favourites, date))
    }

    /// The work record of `date`; without a record on the server the day stays
    /// empty and [`Rolx::add_entry`] creates it. Blocks.
    pub fn day(&self, date: NaiveDate) -> Result<Day, Error> {
        let (begin, end) = day_range(date);
        let runtime = runtime()?;
        let records = runtime
            .block_on(
                self.client
                    .get_work_record_user_id_range_begin_date_end_date(
                        &self.user_id,
                        &begin,
                        &end,
                    ),
            )
            .map_err(api_error)?
            .into_inner();
        // The range holds one day and normally one record.
        let record = records
            .iter()
            .find(|record| record.date.as_deref() == Some(begin.as_str()))
            .or_else(|| if records.len() == 1 { records.first() } else { None })
            .cloned();
        Ok(Day { date, record })
    }

    /// Append an entry to `day` and store the whole day at Rolx. Blocks.
    ///
    /// The server replaces the record of that day with the one sent, so the
    /// entries that are already there are kept. A day the server has no record
    /// for is created with the new entry. `duration_seconds` is the length of
    /// the work, `begin_seconds` its start in seconds since midnight.
    pub fn add_entry(
        &self,
        day: &Day,
        activity: &Activity,
        duration_seconds: i64,
        begin_seconds: i64,
    ) -> Result<(), Error> {
        let mut record = day
            .record
            .clone()
            .unwrap_or_else(|| new_record(self.user_id, day.date, Vec::new()));
        record
            .entries
            .get_or_insert_with(Vec::new)
            .push(record_entry(activity, duration_seconds, begin_seconds));
        let date = date_text(day.date);
        let runtime = runtime()?;
        stored(
            runtime.block_on(
                self.client
                    .put_work_record_user_id_date(&self.user_id, &date, &record),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use api::ClientInfo;
    use chrono::NaiveDate;

    fn activity() -> Activity {
        Activity {
            id: 42,
            number: Some("1234.010".into()),
            label: "1234.010 Consulting".into(),
            starts: None,
            ends: None,
            favourite: false,
        }
    }

    /// A server activity as the API returns it.
    fn server(id: i32, name: &str, starts: Option<&str>, ends: Option<&str>) -> ServerActivity {
        api::types::RolXServerProjectsWebApiResourceActivity {
            id: Some(id),
            full_number: Some(id.to_string()),
            name: Some(name.into()),
            start_date: starts.map(Into::into),
            end_date: ends.map(Into::into),
            ..Default::default()
        }
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 31).unwrap()
    }

    #[test]
    fn merges_the_favourites_and_sorts_by_name() {
        let suitable = vec![server(2, "Beta", None, None), server(1, "Alpha", None, None)];
        let favourites = vec![server(3, "Gamma", None, None)];

        let merged = merge(&suitable, &favourites, today());

        let labels: Vec<&str> = merged.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(labels, ["1 Alpha", "2 Beta", "3 Gamma"]);
        let is_favourite: Vec<bool> = merged.iter().map(|a| a.favourite).collect();
        assert_eq!(is_favourite, [false, false, true]);
    }

    #[test]
    fn a_favourite_that_is_relevant_appears_once() {
        let suitable = vec![server(1, "Alpha", None, None)];
        let favourites = vec![server(1, "Alpha", None, None)];

        let merged = merge(&suitable, &favourites, today());

        assert_eq!(merged.len(), 1);
        assert!(merged[0].favourite);
    }

    #[test]
    fn favourites_outside_their_open_window_are_dropped() {
        let suitable = vec![server(1, "Alpha", None, None)];
        let favourites = vec![
            server(2, "Closed", None, Some("2026-10-30")),
            server(3, "Later", Some("2026-11-01"), None),
            server(4, "OpenToday", Some("2026-10-01"), Some("2026-10-31")),
        ];

        let merged = merge(&suitable, &favourites, today());

        let labels: Vec<&str> = merged.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(labels, ["1 Alpha", "4 OpenToday"]);
    }

    #[test]
    fn an_activity_is_open_within_its_bounds() {
        let bounded = Activity {
            starts: Some(NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()),
            ends: Some(today()),
            ..activity()
        };
        assert!(!bounded.is_open_on(NaiveDate::from_ymd_opt(2026, 9, 30).unwrap()));
        assert!(bounded.is_open_on(NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()));
        assert!(bounded.is_open_on(today()));
        assert!(!bounded.is_open_on(NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()));

        // without bounds the activity is always open
        assert!(activity().is_open_on(today()));
    }

    #[test]
    fn unreadable_bounds_count_as_unbounded() {
        assert_eq!(iso_day(None), None);
        assert_eq!(iso_day(Some("")), None);
        assert_eq!(iso_day(Some("31.10.2026")), None);
        assert_eq!(iso_day(Some(" 2026-10-31 ")), Some(today()));
    }

    #[test]
    fn dates_are_zero_padded_iso() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 7).unwrap();
        assert_eq!(date_text(date), "2026-03-07");
    }

    #[test]
    fn the_day_range_is_half_open_and_one_day_long() {
        let date = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
        assert_eq!(day_range(date), ("2026-10-31".into(), "2026-11-01".into()));
        let last_of_year = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
        assert_eq!(
            day_range(last_of_year),
            ("2026-12-31".into(), "2027-01-01".into())
        );
    }

    #[test]
    fn labels_the_activity() {
        assert_eq!(activity_label(Some("1234"), Some("Consulting"), 1), "1234 Consulting");
        assert_eq!(activity_label(Some("1234"), None, 1), "1234");
        assert_eq!(activity_label(None, Some("Consulting"), 1), "Consulting");
        assert_eq!(activity_label(None, None, 7), "#7");
        assert_eq!(activity_label(Some(" "), Some(""), 7), "#7");
    }

    #[test]
    fn builds_the_record_entry_in_seconds() {
        // 71 min 29 s of work, started at 11:46:11.
        let entry = record_entry(&activity(), 4289, 11 * 3600 + 46 * 60 + 11);
        assert_eq!(entry.activity_id, Some(42));
        assert_eq!(entry.full_activity_number.as_deref(), Some("1234.010"));
        assert_eq!(entry.duration, Some(4289));
        assert_eq!(entry.begin, Some(42371));
        assert_eq!(entry.pause, None);
        assert_eq!(entry.comment, None);
    }

    #[test]
    fn the_entry_is_sent_the_way_rolx_expects_it() {
        let entry = record_entry(&activity(), 4289, 42371);
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "activityId": 42,
                "fullActivityNumber": "1234.010",
                "duration": 4289,
                "begin": 42371,
            }),
            "Rolx reads duration and begin in seconds; pause stays unset"
        );
    }

    #[test]
    fn sums_the_recorded_seconds() {
        let day = Day {
            date: NaiveDate::from_ymd_opt(2026, 10, 31).unwrap(),
            record: Some(api::types::RolXServerRecordsWebApiResourceRecord {
                entries: Some(vec![
                    record_entry(&activity(), 30 * 60, 8 * 3600),
                    record_entry(&activity(), 45 * 60, 10 * 3600),
                ]),
                ..Default::default()
            }),
        };
        assert_eq!(day.recorded_seconds(), 75 * 60);
        assert!(day.has_record());

        let empty = Day {
            date: day.date,
            record: Some(Default::default()),
        };
        assert_eq!(empty.recorded_seconds(), 0);
    }

    #[test]
    fn a_day_without_a_record_is_empty() {
        let day = Day {
            date: NaiveDate::from_ymd_opt(2026, 10, 31).unwrap(),
            record: None,
        };
        assert_eq!(day.recorded_seconds(), 0);
        assert!(!day.has_record());
    }

    #[test]
    fn a_new_record_carries_date_user_and_entries() {
        let user_id = Uuid::nil();
        let date = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
        let entries = vec![record_entry(&activity(), 72 * 60, 8 * 3600 + 5 * 60)];
        let record = new_record(user_id, date, entries);

        assert_eq!(record.date.as_deref(), Some("2026-10-31"));
        assert_eq!(record.user_id, Some(user_id));
        assert_eq!(record.entries.map(|entries| entries.len()), Some(1));
        // The server decides the rest of the day.
        assert!(record.day_type.is_none());
        assert!(record.day_name.is_none());
        assert!(record.nominal_work_time.is_none());
        assert!(record.paid_leave_type.is_none());
        assert!(record.paid_leave_reason.is_none());
    }

    #[test]
    fn any_success_status_counts_as_stored() {
        // Rolx answers the update with 204 while the spec declares 200.
        assert!(accepted_status(Some(200)));
        assert!(accepted_status(Some(204)));
        assert!(accepted_status(Some(299)));
        assert!(!accepted_status(Some(300)));
        assert!(!accepted_status(Some(400)));
        assert!(!accepted_status(Some(500)));
        assert!(!accepted_status(None));
    }

    #[test]
    fn client_rejects_an_invalid_token_without_leaking_it() {
        let err = api::Client::create("https://example.invalid/", "bad\ntoken")
            .expect_err("invalid token must be rejected");
        assert!(!err.to_string().contains("bad"));
    }

    #[test]
    fn client_trims_the_trailing_slash() {
        let client = api::Client::create("https://example.invalid/", "token").unwrap();
        assert_eq!(client.baseurl(), "https://example.invalid");
    }
}
