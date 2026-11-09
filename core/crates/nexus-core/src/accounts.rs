//! Perfis, PINs, controle parental e tempo de uso. A Shell nunca decide permissões: o Core aplica.
use crate::db::Db;
use crate::util;
use argon2::Argon2;
use nexus_protocol::*;
use rusqlite::params;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq)]
pub enum PinError {
    Required,
    Invalid { remaining: u32 },
    Locked { secs: u64 },
    Malformed,
}

const MAX_TRIES: u32 = 5;
const LOCK: Duration = Duration::from_secs(60);
const GRANT_TTL: Duration = Duration::from_secs(600);

pub struct Accounts {
    db: Arc<Db>,
    fails: Mutex<HashMap<String, (u32, Option<Instant>)>>,
    grants: Mutex<HashMap<String, Instant>>,
}

fn hash_pin(pin: &str, salt: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; 32];
    let _ = Argon2::default().hash_password_into(pin.as_bytes(), salt, &mut out);
    out
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn valid_pin(pin: &str) -> bool {
    (4..=6).contains(&pin.len()) && pin.chars().all(|c| c.is_ascii_digit())
}

#[derive(Default, Debug, Clone)]
pub struct NewProfile {
    pub name: String,
    pub avatar: String,
    pub hue: u16,
    pub kid: bool,
    pub pin: Option<String>,
    pub age_limit: Option<u8>,
    pub daily_limit_min: Option<u32>,
}

#[derive(Default, Debug, Clone)]
pub struct ProfilePatch {
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub hue: Option<u16>,
    pub age_limit: Option<Option<u8>>,
    pub daily_limit_min: Option<Option<u32>>,
    /// Some(Some(pin)) define, Some(None) remove
    pub pin: Option<Option<String>>,
}

fn row_profile(r: &rusqlite::Row) -> rusqlite::Result<Profile> {
    let kind: String = r.get("kind")?;
    let pin: Option<Vec<u8>> = r.get("pin_hash")?;
    Ok(Profile {
        id: r.get("id")?,
        name: r.get("name")?,
        avatar: r.get("avatar")?,
        hue: r.get::<_, i64>("hue")? as u16,
        kind: if kind == "kid" { ProfileKind::Kid } else { ProfileKind::Adult },
        has_pin: pin.is_some(),
        age_limit: r.get::<_, Option<i64>>("age_limit")?.map(|v| v as u8),
        daily_limit_min: r.get::<_, Option<i64>>("daily_limit_min")?.map(|v| v as u32),
    })
}

impl Accounts {
    pub fn new(db: Arc<Db>) -> Accounts {
        Accounts { db, fails: Mutex::new(HashMap::new()), grants: Mutex::new(HashMap::new()) }
    }

    pub fn list(&self) -> anyhow::Result<Vec<Profile>> {
        self.db.with(|c| {
            let mut st = c.prepare("SELECT * FROM profiles ORDER BY position, created_at")?;
            let v = st.query_map([], row_profile)?.collect::<Result<Vec<_>, _>>()?;
            Ok(v)
        })
    }

    pub fn get(&self, id: &str) -> Option<Profile> {
        self.db.with(|c| c.query_row("SELECT * FROM profiles WHERE id=?1", params![id], row_profile)).ok()
    }

    pub fn create(&self, n: NewProfile) -> anyhow::Result<Profile> {
        let name = n.name.trim().to_string();
        if name.is_empty() || name.chars().count() > 16 {
            anyhow::bail!("nome inválido");
        }
        if self.list()?.iter().any(|p| p.name.to_lowercase() == name.to_lowercase()) {
            anyhow::bail!("já existe um perfil com esse nome");
        }
        if let Some(p) = &n.pin {
            if !valid_pin(p) {
                anyhow::bail!("PIN deve ter de 4 a 6 números");
            }
        }
        let id = format!("p{}", util::random_hex(8));
        let salt = util::random_bytes::<16>();
        let (hash, salt_v) = match &n.pin {
            Some(p) => (Some(hash_pin(p, &salt)), Some(salt.to_vec())),
            None => (None, None),
        };
        let prefs = serde_json::to_string(&Prefs::default())?;
        let pos = self.list()?.len() as i64;
        let age = if n.kid { Some(n.age_limit.unwrap_or(10)) } else { None };
        self.db.with(|c| {
            c.execute(
                "INSERT INTO profiles(id,name,avatar,hue,kind,pin_hash,pin_salt,age_limit,daily_limit_min,prefs,created_at,position)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![id, name, n.avatar, n.hue as i64, if n.kid { "kid" } else { "adult" }, hash, salt_v, age.map(|v| v as i64), n.daily_limit_min.map(|v| v as i64), prefs, now_unix(), pos],
            )
            .map(|_| ())
        })?;
        Ok(self.get(&id).expect("perfil recém criado"))
    }

    pub fn update(&self, id: &str, p: ProfilePatch) -> anyhow::Result<Profile> {
        let cur = self.get(id).ok_or_else(|| anyhow::anyhow!("perfil não existe"))?;
        if let Some(n) = &p.name {
            let n = n.trim();
            if n.is_empty() || n.chars().count() > 16 {
                anyhow::bail!("nome inválido");
            }
            if self.list()?.iter().any(|o| o.id != id && o.name.to_lowercase() == n.to_lowercase()) {
                anyhow::bail!("já existe um perfil com esse nome");
            }
        }
        self.db.with(|c| {
            if let Some(n) = &p.name {
                c.execute("UPDATE profiles SET name=?2 WHERE id=?1", params![id, n.trim()])?;
            }
            if let Some(a) = &p.avatar {
                c.execute("UPDATE profiles SET avatar=?2 WHERE id=?1", params![id, a])?;
            }
            if let Some(h) = p.hue {
                c.execute("UPDATE profiles SET hue=?2 WHERE id=?1", params![id, h as i64])?;
            }
            if let Some(a) = p.age_limit {
                c.execute("UPDATE profiles SET age_limit=?2 WHERE id=?1", params![id, a.map(|v| v as i64)])?;
            }
            if let Some(d) = p.daily_limit_min {
                c.execute("UPDATE profiles SET daily_limit_min=?2 WHERE id=?1", params![id, d.map(|v| v as i64)])?;
            }
            Ok(())
        })?;
        if let Some(pin) = p.pin {
            match pin {
                Some(pin) => {
                    if !valid_pin(&pin) {
                        anyhow::bail!("PIN deve ter de 4 a 6 números");
                    }
                    let salt = util::random_bytes::<16>();
                    let h = hash_pin(&pin, &salt);
                    self.db.with(|c| c.execute("UPDATE profiles SET pin_hash=?2, pin_salt=?3 WHERE id=?1", params![id, h, salt.to_vec()]).map(|_| ()))?;
                }
                None => {
                    self.db.with(|c| c.execute("UPDATE profiles SET pin_hash=NULL, pin_salt=NULL WHERE id=?1", params![id]).map(|_| ()))?;
                }
            }
        }
        let _ = cur;
        Ok(self.get(id).expect("perfil existe"))
    }

    pub fn delete(&self, id: &str) -> anyhow::Result<()> {
        self.db.with(|c| {
            c.execute("DELETE FROM profiles WHERE id=?1", params![id])?;
            c.execute("DELETE FROM playtime WHERE profile_id=?1", params![id])?;
            c.execute("DELETE FROM usage WHERE profile_id=?1", params![id])?;
            Ok(())
        })
    }

    fn check_lock(&self, key: &str) -> Result<(), PinError> {
        let mut f = self.fails.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, Some(until))) = f.get(key) {
            let now = Instant::now();
            if *until > now {
                return Err(PinError::Locked { secs: (*until - now).as_secs() + 1 });
            }
            f.remove(key);
        }
        Ok(())
    }

    fn fail(&self, key: &str) -> PinError {
        let mut f = self.fails.lock().unwrap_or_else(|e| e.into_inner());
        let e = f.entry(key.to_string()).or_insert((0, None));
        e.0 += 1;
        if e.0 >= MAX_TRIES {
            e.1 = Some(Instant::now() + LOCK);
            e.0 = 0;
            return PinError::Locked { secs: LOCK.as_secs() };
        }
        PinError::Invalid { remaining: MAX_TRIES - e.0 }
    }

    fn ok(&self, key: &str) {
        self.fails.lock().unwrap_or_else(|e| e.into_inner()).remove(key);
    }

    fn stored(&self, id: &str) -> Option<(Vec<u8>, Vec<u8>)> {
        self.db
            .with(|c| c.query_row("SELECT pin_hash, pin_salt FROM profiles WHERE id=?1", params![id], |r| Ok((r.get::<_, Option<Vec<u8>>>(0)?, r.get::<_, Option<Vec<u8>>>(1)?))))
            .ok()
            .and_then(|(h, s)| Some((h?, s?)))
    }

    /// Ok se o perfil não tem PIN ou se o PIN confere.
    pub fn verify_profile_pin(&self, id: &str, pin: Option<&str>) -> Result<(), PinError> {
        let Some((hash, salt)) = self.stored(id) else { return Ok(()) };
        let key = format!("profile:{id}");
        self.check_lock(&key)?;
        let Some(pin) = pin.filter(|p| !p.is_empty()) else { return Err(PinError::Required) };
        if !valid_pin(pin) {
            return Err(PinError::Malformed);
        }
        if ct_eq(&hash_pin(pin, &salt), &hash) {
            self.ok(&key);
            Ok(())
        } else {
            Err(self.fail(&key))
        }
    }

    // ---- PIN dos responsáveis (um por aparelho) ----
    pub fn parental(&self) -> Parental {
        Parental { has_pin: self.db.kv_get("parental_pin").is_some() }
    }

    pub fn verify_parental(&self, pin: Option<&str>) -> Result<(), PinError> {
        let Some(stored) = self.db.kv_get("parental_pin") else { return Ok(()) };
        self.check_lock("parental")?;
        let Some(pin) = pin.filter(|p| !p.is_empty()) else { return Err(PinError::Required) };
        if !valid_pin(pin) {
            return Err(PinError::Malformed);
        }
        let (h, s) = stored.split_once(':').unwrap_or(("", ""));
        if ct_eq(&hash_pin(pin, &unhex(s)), &unhex(h)) {
            self.ok("parental");
            Ok(())
        } else {
            Err(self.fail("parental"))
        }
    }

    pub fn set_parental(&self, new_pin: Option<&str>, current: Option<&str>) -> Result<(), PinError> {
        self.verify_parental(current)?;
        match new_pin {
            Some(p) => {
                if !valid_pin(p) {
                    return Err(PinError::Malformed);
                }
                let salt = util::random_bytes::<16>();
                let v = format!("{}:{}", hex(&hash_pin(p, &salt)), hex(&salt));
                let _ = self.db.kv_set("parental_pin", &v);
            }
            None => {
                let _ = self.db.kv_del("parental_pin");
            }
        }
        Ok(())
    }

    /// Valida o PIN dos responsáveis e devolve uma autorização curta reutilizável.
    pub fn grant(&self, pin: Option<&str>) -> Result<String, PinError> {
        self.verify_parental(pin)?;
        let t = util::random_hex(24);
        let mut g = self.grants.lock().unwrap_or_else(|e| e.into_inner());
        g.retain(|_, at| at.elapsed() < GRANT_TTL);
        g.insert(t.clone(), Instant::now());
        Ok(t)
    }

    pub fn grant_valid(&self, token: Option<&str>) -> bool {
        let Some(t) = token else { return false };
        let mut g = self.grants.lock().unwrap_or_else(|e| e.into_inner());
        g.retain(|_, at| at.elapsed() < GRANT_TTL);
        g.contains_key(t)
    }

    /// Sem PIN dos responsáveis configurado, a autorização é implícita.
    pub fn parental_ok(&self, pin: Option<&str>, grant: Option<&str>) -> Result<(), PinError> {
        if !self.parental().has_pin || self.grant_valid(grant) {
            return Ok(());
        }
        self.verify_parental(pin)
    }

    // ---- preferências e itens liberados ----
    pub fn prefs(&self, id: &str) -> Prefs {
        self.db
            .with(|c| c.query_row("SELECT prefs FROM profiles WHERE id=?1", params![id], |r| r.get::<_, String>(0)))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn set_prefs(&self, id: &str, prefs: &Prefs) -> anyhow::Result<()> {
        let s = serde_json::to_string(prefs)?;
        self.db.with(|c| c.execute("UPDATE profiles SET prefs=?2 WHERE id=?1", params![id, s]).map(|_| ()))
    }

    pub fn allowed(&self, id: &str) -> Vec<ItemId> {
        self.db
            .with(|c| c.query_row("SELECT allowed FROM profiles WHERE id=?1", params![id], |r| r.get::<_, String>(0)))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn set_allowed(&self, id: &str, item: &str, allow: bool) -> anyhow::Result<Vec<ItemId>> {
        let mut a = self.allowed(id);
        a.retain(|x| x != item);
        if allow {
            a.push(item.to_string());
        }
        let s = serde_json::to_string(&a)?;
        self.db.with(|c| c.execute("UPDATE profiles SET allowed=?2 WHERE id=?1", params![id, s]).map(|_| ()))?;
        Ok(a)
    }

    // ---- uso diário e tempo de jogo ----
    pub fn add_usage(&self, profile: &str, secs: u64) {
        let day = util::today();
        let _ = self.db.with(|c| {
            c.execute(
                "INSERT INTO usage(profile_id,day,seconds) VALUES(?1,?2,?3) ON CONFLICT(profile_id,day) DO UPDATE SET seconds=seconds+?3",
                params![profile, day, secs as i64],
            )
            .map(|_| ())
        });
    }

    pub fn extend_today(&self, profile: &str, minutes: u32) {
        let day = util::today();
        let _ = self.db.with(|c| {
            c.execute(
                "INSERT INTO usage(profile_id,day,seconds,bonus) VALUES(?1,?2,0,?3) ON CONFLICT(profile_id,day) DO UPDATE SET bonus=bonus+?3",
                params![profile, day, minutes as i64 * 60],
            )
            .map(|_| ())
        });
    }

    /// Minutos restantes hoje (None = sem limite).
    pub fn remaining_min(&self, p: &Profile) -> Option<u32> {
        let limit = p.daily_limit_min? as i64 * 60;
        let day = util::today();
        let (used, bonus) = self
            .db
            .with(|c| c.query_row("SELECT seconds, bonus FROM usage WHERE profile_id=?1 AND day=?2", params![p.id, day], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))))
            .unwrap_or((0, 0));
        Some(((limit + bonus - used).max(0) as u64).div_ceil(60) as u32)
    }

    pub fn add_playtime(&self, profile: &str, item: &str, secs: u64) {
        let _ = self.db.with(|c| {
            c.execute(
                "INSERT INTO playtime(profile_id,item_id,seconds,last_played) VALUES(?1,?2,?3,?4)
                 ON CONFLICT(profile_id,item_id) DO UPDATE SET seconds=seconds+?3, last_played=?4",
                params![profile, item, secs as i64, now_unix()],
            )
            .map(|_| ())
        });
    }

    pub fn playtime(&self, profile: &str, item: &str) -> (u64, Option<i64>) {
        self.db
            .with(|c| c.query_row("SELECT seconds,last_played FROM playtime WHERE profile_id=?1 AND item_id=?2", params![profile, item], |r| Ok((r.get::<_, i64>(0)? as u64, r.get::<_, Option<i64>>(1)?))))
            .unwrap_or((0, None))
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).filter_map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acc() -> Accounts {
        Accounts::new(Arc::new(Db::memory().unwrap()))
    }

    fn np(name: &str) -> NewProfile {
        NewProfile { name: name.into(), avatar: "initial".into(), hue: 180, ..Default::default() }
    }

    #[test]
    fn create_list_update_delete() {
        let a = acc();
        let p = a.create(np("Lia")).unwrap();
        assert_eq!(p.kind, ProfileKind::Adult);
        assert!(a.create(np("lia")).is_err(), "nome duplicado (sem diferenciar maiúsculas)");
        assert!(a.create(np("  ")).is_err());
        let p2 = a.update(&p.id, ProfilePatch { name: Some("Lia M".into()), ..Default::default() }).unwrap();
        assert_eq!(p2.name, "Lia M");
        a.delete(&p.id).unwrap();
        assert!(a.list().unwrap().is_empty());
    }

    #[test]
    fn profile_pin_and_lockout() {
        let a = acc();
        let p = a.create(NewProfile { pin: Some("1234".into()), ..np("Teo") }).unwrap();
        assert!(p.has_pin);
        assert_eq!(a.verify_profile_pin(&p.id, None), Err(PinError::Required));
        assert!(a.verify_profile_pin(&p.id, Some("1234")).is_ok());
        for i in 1..5 {
            assert_eq!(a.verify_profile_pin(&p.id, Some("0000")), Err(PinError::Invalid { remaining: 5 - i }));
        }
        assert!(matches!(a.verify_profile_pin(&p.id, Some("0000")), Err(PinError::Locked { .. })));
        assert!(matches!(a.verify_profile_pin(&p.id, Some("1234")), Err(PinError::Locked { .. })), "bloqueado mesmo com PIN certo");
        assert!(a.create(NewProfile { pin: Some("12".into()), ..np("X") }).is_err());
    }

    #[test]
    fn parental_pin_and_grants() {
        let a = acc();
        assert!(!a.parental().has_pin);
        assert!(a.parental_ok(None, None).is_ok(), "sem PIN configurado tudo é liberado");
        a.set_parental(Some("4321"), None).unwrap();
        assert!(a.parental().has_pin);
        assert_eq!(a.parental_ok(None, None), Err(PinError::Required));
        let g = a.grant(Some("4321")).unwrap();
        assert!(a.parental_ok(None, Some(&g)).is_ok());
        assert!(a.parental_ok(None, Some("falso")).is_err());
        assert_eq!(a.set_parental(Some("1111"), Some("9999")), Err(PinError::Invalid { remaining: 4 }));
        a.set_parental(Some("1111"), Some("4321")).unwrap();
        a.set_parental(None, Some("1111")).unwrap();
        assert!(!a.parental().has_pin);
    }

    #[test]
    fn kid_limits_and_usage() {
        let a = acc();
        let k = a.create(NewProfile { kid: true, daily_limit_min: Some(30), age_limit: Some(10), ..np("Pipo") }).unwrap();
        assert_eq!(k.kind, ProfileKind::Kid);
        assert_eq!(a.remaining_min(&k), Some(30));
        a.add_usage(&k.id, 600);
        assert_eq!(a.remaining_min(&k), Some(20));
        a.add_usage(&k.id, 3600);
        assert_eq!(a.remaining_min(&k), Some(0));
        a.extend_today(&k.id, 15);
        assert_eq!(a.remaining_min(&k), Some(0), "usou 70 min de 45");
        a.extend_today(&k.id, 60);
        assert_eq!(a.remaining_min(&k), Some(35), "30 + 75 de bônus - 70 usados");
        let adult = a.create(np("Mãe")).unwrap();
        assert_eq!(a.remaining_min(&adult), None);
    }

    #[test]
    fn prefs_allowed_playtime() {
        let a = acc();
        let p = a.create(np("Rafa")).unwrap();
        let mut pr = a.prefs(&p.id);
        assert_eq!(pr.theme, "dark");
        pr.theme = "light".into();
        pr.pins.push("steam:1".into());
        a.set_prefs(&p.id, &pr).unwrap();
        assert_eq!(a.prefs(&p.id).theme, "light");
        assert_eq!(a.set_allowed(&p.id, "steam:1", true).unwrap(), vec!["steam:1".to_string()]);
        assert!(a.set_allowed(&p.id, "steam:1", false).unwrap().is_empty());
        a.add_playtime(&p.id, "steam:1", 90);
        a.add_playtime(&p.id, "steam:1", 30);
        assert_eq!(a.playtime(&p.id, "steam:1").0, 120);
    }
}
