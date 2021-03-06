use https_everywhere_lib_core::{updater::{UpdateChannels, Updater}, RuleSets, rewriter::{Rewriter, RewriteAction}, Storage, Settings};
use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;
use std::fs;
use std::sync::{Arc, Mutex};
use rusqlite::NO_PARAMS;
use rusqlite::{params, Connection, OptionalExtension};
use url::Host;

use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

pub struct SQLiteStorage {
    conn: Mutex<Connection>,
}

impl SQLiteStorage {
    fn new() -> SQLiteStorage {
        let conn_mutex = Mutex::new(Connection::open(".https-everywhere-storage.db").unwrap());
        {
            let conn = conn_mutex.lock().unwrap();
            conn.execute("
                CREATE TABLE IF NOT EXISTS `ints` (
                    `key` STRING PRIMARY KEY,
                    `value` INTEGER NOT NULL
                )",
                NO_PARAMS,
            ).unwrap();
            conn.execute("
                CREATE TABLE IF NOT EXISTS `bools` (
                    `key` STRING PRIMARY KEY,
                    `value` BOOL NOT NULL
                )",
                NO_PARAMS,
            ).unwrap();
            conn.execute("
                CREATE TABLE IF NOT EXISTS `strings` (
                    `key` STRING PRIMARY KEY,
                    `value` TEXT NOT NULL
                )",
                NO_PARAMS,
            ).unwrap();
        }
        SQLiteStorage {
            conn: conn_mutex
        }
    }
}

impl Storage for SQLiteStorage {
    fn get_int(&self, key: String) -> Option<usize> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT `value` FROM `ints` WHERE `key` = ?1",
            params![key],
            |row| {
                match row.get::<_, isize>(0) {
                    Ok(integer) => Ok(integer as usize),
                    Err(err) => Err(err)
                }
            }
        ).optional().unwrap()
    }

    fn get_bool(&self, key: String) -> Option<bool> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT `value` FROM `bools` WHERE `key` = ?1",
            params![key],
            |row| row.get(0)
        ).optional().unwrap()
    }

    fn get_string(&self, key: String) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT `value` FROM `strings` WHERE `key` = ?1",
            params![key],
            |row| row.get(0)
        ).optional().unwrap()
    }

    fn set_int(&mut self, key: String, value: usize) {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO `ints` (`key`, `value`) VALUES (?1, ?2) ON CONFLICT (`key`) DO UPDATE SET `value`=`excluded`.`value`",
            params![key, value as isize],
        ).unwrap();
    }

    fn set_bool(&mut self, key: String, value: bool) {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO `bools` (`key`, `value`) VALUES (?1, ?2) ON CONFLICT (`key`) DO UPDATE SET `value`=`excluded`.`value`",
            params![key, value],
        ).unwrap();
    }

    fn set_string(&mut self, key: String, value: String) {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO `strings` (`key`, `value`) VALUES (?1, ?2) ON CONFLICT (`key`) DO UPDATE SET `value`=`excluded`.`value`",
            params![key, value],
        ).unwrap();
    }
}

/// Formats the sum of two numbers as string.
#[pyfunction]
fn create_rulesets() -> PyResult<usize> {
    let rs = RuleSets::new();
    Ok(Box::into_raw(Box::new(Arc::new(Mutex::new(rs)))) as usize)
}

#[pyfunction]
unsafe fn destroy_rulesets(ptr: usize) {
    drop(Box::from_raw(ptr as *mut Arc<Mutex<RuleSets>>));
}

#[pyfunction]
fn create_storage() -> PyResult<usize> {
    let s = SQLiteStorage::new();
    Ok(Box::into_raw(Box::new(Arc::new(Mutex::new(s)))) as usize)
}

#[pyfunction]
unsafe fn destroy_storage(ptr: usize) {
    drop(Box::from_raw(ptr as *mut Arc<Mutex<SQLiteStorage>>));
}

#[pyfunction]
unsafe fn create_rewriter(rulesets_ptr: usize, settings_ptr: usize) -> PyResult<usize> {
    let rs = & *(rulesets_ptr as *mut Arc<Mutex<RuleSets>>);
    let settings = & *(settings_ptr as *mut Arc<Mutex<Settings>>);

    let rs_threadsafe = Arc::clone(rs);
    let settings_threadsafe = Arc::clone(settings);

    let rw = Rewriter::new(rs_threadsafe, settings_threadsafe);
    Ok(Box::into_raw(Box::new(rw)) as usize)
}

#[pyfunction]
unsafe fn destroy_rewriter(ptr: usize) {
    drop(Box::from_raw(ptr as *mut Rewriter));
}

#[pyfunction]
unsafe fn create_settings(storage_ptr: usize) -> PyResult<usize> {
    let s = & *(storage_ptr as *mut Arc<Mutex<SQLiteStorage>>);

    let s_threadsafe = Arc::clone(s);

    let settings = Settings::new(s_threadsafe);
    Ok(Box::into_raw(Box::new(Arc::new(Mutex::new(settings)))) as usize)
}

#[pyfunction]
unsafe fn destroy_settings(ptr: usize) {
    drop(Box::from_raw(ptr as *mut Arc<Mutex<Settings>>));
}

#[pyfunction]
unsafe fn create_updater(rulesets_ptr: usize, storage_ptr: usize) -> PyResult<usize> {
    let rs = & *(rulesets_ptr as *mut Arc<Mutex<RuleSets>>);
    let s = & *(storage_ptr as *mut Arc<Mutex<SQLiteStorage>>);

    let update_channels_string = fs::read_to_string("update_channels.json").unwrap();
    let ucs = UpdateChannels::from(&update_channels_string[..]);

    let rs_threadsafe = Arc::clone(rs);
    let s_threadsafe = Arc::clone(s);

    let updater = Updater::new(rs_threadsafe, ucs, s_threadsafe, None, 15);
    Ok(Box::into_raw(Box::new(updater)) as usize)
}

#[pyfunction]
unsafe fn destroy_updater(ptr: usize) {
    drop(Box::from_raw(ptr as *mut Updater));
}




#[pyfunction]
unsafe fn update_rulesets(ptr: usize) {
    let updater = &mut *(ptr as *mut Updater);
    updater.apply_stored_rulesets();
    updater.perform_check();
}

#[pyfunction]
unsafe fn get_update_channel_timestamps(updater_ptr: usize) -> PyResult<HashMap<String, Option<usize>>> {
    let updater = & *(updater_ptr as *mut Updater);
    Ok(updater.get_update_channel_timestamps())
}

#[pyfunction]
unsafe fn rewrite_url(ptr: usize, url: String) -> PyResult<(bool, bool, String, bool)> {
    let rw = &mut *(ptr as *mut Rewriter);

    if let Ok(ra) = rw.rewrite_url(&url) {
	match ra {
	    RewriteAction::CancelRequest => {
                return Ok((true, false, "".to_string(), false));
            },
            RewriteAction::NoOp => {
                return Ok((false, true, "".to_string(), false));
	    },
	    RewriteAction::RewriteUrl(url) => {
                return Ok((false, false, url, false));
	    },
	    RewriteAction::RedirectLoopWarning => {
                return Ok((false, false, "".to_string(), true));
	    },
	}
    } else {
	panic!("An error occurred attempting to rewrite url: {}", url);
    }
}

#[pyfunction]
unsafe fn get_enabled_or(ptr: usize, default: bool) -> PyResult<bool> {
    let settings = & *(ptr as *mut Arc<Mutex<Settings>>);
    Ok(settings.lock().unwrap().get_https_everywhere_enabled_or(default))
}

#[pyfunction]
unsafe fn set_enabled(ptr: usize, value: bool) {
    let settings = &mut *(ptr as *mut Arc<Mutex<Settings>>);
    settings.lock().unwrap().set_https_everywhere_enabled(value)
}

#[pyfunction]
unsafe fn get_ease_mode_enabled_or(ptr: usize, default: bool) -> PyResult<bool> {
    let settings = & *(ptr as *mut Arc<Mutex<Settings>>);
    Ok(settings.lock().unwrap().get_ease_mode_enabled_or(default))
}

#[pyfunction]
unsafe fn set_ease_mode_enabled(ptr: usize, value: bool) {
    let settings = &mut *(ptr as *mut Arc<Mutex<Settings>>);
    settings.lock().unwrap().set_ease_mode_enabled(value)
}

#[pyfunction]
unsafe fn get_sites_disabled(ptr: usize) -> PyResult<HashSet<String>> {
    let settings = & *(ptr as *mut Arc<Mutex<Settings>>);
    Ok(HashSet::from_iter(settings.lock().unwrap().get_sites_disabled().iter().map(|host| host.to_string())))
}

#[pyfunction]
unsafe fn set_site_disabled(ptr: usize, site: String, set_disabled: bool) -> PyResult<bool> {
    let settings = &mut *(ptr as *mut Arc<Mutex<Settings>>);
    settings.lock().unwrap().set_site_disabled(match Host::parse(&site) {
        Ok(res) => res,
        Err(_) => return Ok(false),
    }, set_disabled);
    Ok(true)
}



/// A Python module implemented in Rust.
#[pymodule]
fn https_everywhere_standalone_pyo(_py: Python, m: &PyModule) -> PyResult<()> {
    simple_logger::init().unwrap();
    m.add_wrapped(wrap_pyfunction!(create_rulesets))?;
    m.add_wrapped(wrap_pyfunction!(destroy_rulesets))?;
    m.add_wrapped(wrap_pyfunction!(create_storage))?;
    m.add_wrapped(wrap_pyfunction!(destroy_storage))?;
    m.add_wrapped(wrap_pyfunction!(create_rewriter))?;
    m.add_wrapped(wrap_pyfunction!(destroy_rewriter))?;
    m.add_wrapped(wrap_pyfunction!(create_settings))?;
    m.add_wrapped(wrap_pyfunction!(destroy_settings))?;
    m.add_wrapped(wrap_pyfunction!(create_updater))?;
    m.add_wrapped(wrap_pyfunction!(destroy_updater))?;

    m.add_wrapped(wrap_pyfunction!(update_rulesets))?;
    m.add_wrapped(wrap_pyfunction!(get_update_channel_timestamps))?;
    m.add_wrapped(wrap_pyfunction!(rewrite_url))?;
    m.add_wrapped(wrap_pyfunction!(get_enabled_or))?;
    m.add_wrapped(wrap_pyfunction!(set_enabled))?;
    m.add_wrapped(wrap_pyfunction!(get_ease_mode_enabled_or))?;
    m.add_wrapped(wrap_pyfunction!(set_ease_mode_enabled))?;
    m.add_wrapped(wrap_pyfunction!(get_sites_disabled))?;
    m.add_wrapped(wrap_pyfunction!(set_site_disabled))?;

    Ok(())
}
