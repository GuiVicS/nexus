//! Estado do aparelho para a barra de status: rede, Bluetooth (controles com bateria), som e espaço em disco, mais
//! os alertas (sem internet, bateria de controle <= 20 %, disco com menos de 10 % livre). Uma única tarefa consulta
//! os serviços e só emite `status.changed` quando algo muda — a Shell não faz consultas periódicas.
//! `NEXUS_STATUS_SECS` muda o intervalo (padrão 15 s).
use crate::core::Core;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

pub const LOW_BATTERY: u64 = 20;
pub const LOW_DISK_PCT: u64 = 10;

fn net_part(n: &Value) -> Value {
    if !n["available"].as_bool().unwrap_or(false) {
        return json!({ "available": false, "kind": "none", "online": false });
    }
    let main = &n["main"];
    let kind = match main["type"].as_str() {
        Some("wifi") => "wifi",
        Some(_) => "cabo",
        None => "none",
    };
    json!({ "available": true, "kind": kind, "wifiEnabled": n["wifiEnabled"].as_bool().unwrap_or(false), "hasWifi": n["hasWifi"].as_bool().unwrap_or(false), "name": main["connection"], "signal": n["signal"], "online": n["online"].as_bool().unwrap_or(false), "connectivity": n["connectivity"] })
}

fn bt_part(b: &Value) -> Value {
    let connected: Vec<Value> = b["paired"]
        .as_array()
        .map(|l| l.iter().filter(|d| d["connected"].as_bool().unwrap_or(false)).map(|d| json!({ "mac": d["mac"], "name": d["name"], "kind": d["kind"], "battery": d["battery"] })).collect())
        .unwrap_or_default();
    json!({ "available": b["available"].as_bool().unwrap_or(false), "powered": b["powered"].as_bool().unwrap_or(false), "connected": connected })
}

/// Alertas a partir das partes já lidas. Só aparecem quando há problema.
pub fn alerts(net: &Value, bt: &Value, disk: &Value) -> Vec<Value> {
    let mut v = vec![];
    if net["available"].as_bool().unwrap_or(false) && !net["online"].as_bool().unwrap_or(false) {
        let text = if net["kind"] == "none" { "Sem conexão com a rede" } else { "Sem internet" };
        v.push(json!({ "id": "net", "icon": "wifi", "text": text }));
    }
    for d in bt["connected"].as_array().into_iter().flatten() {
        if d["kind"] == "controle" {
            if let Some(b) = d["battery"].as_u64().filter(|b| *b <= LOW_BATTERY) {
                v.push(json!({ "id": format!("pad:{}", d["mac"].as_str().unwrap_or("")), "icon": "pad", "text": format!("{} com pouca bateria ({b}%)", d["name"].as_str().unwrap_or("Controle")) }));
            }
        }
    }
    if let Some(p) = disk["freePct"].as_u64().filter(|p| *p < LOW_DISK_PCT && disk["totalBytes"].as_u64().unwrap_or(0) > 0) {
        v.push(json!({ "id": "disk", "icon": "download", "text": format!("Pouco espaço ({p}% livre)") }));
    }
    v
}

pub fn disk_part(dir: &std::path::Path) -> Value {
    let probe = if dir.exists() { dir } else { std::path::Path::new("/") };
    let (free, total) = crate::hardware::disk(probe);
    json!({ "freeBytes": free, "totalBytes": total, "freePct": if total > 0 { free * 100 / total } else { 100 } })
}

pub fn snapshot(dir: &std::path::Path) -> Value {
    let net = net_part(&crate::sysnet::status());
    let bt = bt_part(&crate::bluetooth::status());
    let audio = match crate::audio::quick() {
        Some((volume, muted)) => json!({ "available": true, "volume": volume, "muted": muted }),
        None => json!({ "available": false, "volume": 0, "muted": false }),
    };
    let disk = disk_part(dir);
    let alerts = alerts(&net, &bt, &disk);
    json!({ "net": net, "bt": bt, "audio": audio, "disk": disk, "alerts": alerts })
}

pub fn spawn(core: Arc<Core>) {
    let secs = std::env::var("NEXUS_STATUS_SECS").ok().and_then(|s| s.parse::<u64>().ok()).unwrap_or(15).max(1);
    tokio::spawn(async move {
        let mut last = Value::Null;
        loop {
            let dir = core.cfg.data_dir.clone();
            if let Ok(cur) = tokio::task::spawn_blocking(move || snapshot(&dir)).await {
                if cur != last {
                    *core.status.lock().unwrap_or_else(|e| e.into_inner()) = cur.clone();
                    core.emit("status.changed", cur.clone());
                    last = cur;
                }
            }
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(secs)) => {}
                _ = core.status_kick.notified() => {}
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alertas_so_com_problema() {
        let net = json!({ "available": true, "kind": "wifi", "online": true });
        let bt = json!({ "connected": [{ "mac": "AA", "name": "Controle", "kind": "controle", "battery": 90 }] });
        let disk = json!({ "freePct": 50, "totalBytes": 100 });
        assert!(alerts(&net, &bt, &disk).is_empty());
        let net = json!({ "available": true, "kind": "wifi", "online": false });
        let bt = json!({ "connected": [{ "mac": "AA", "name": "Pad", "kind": "controle", "battery": 20 }, { "mac": "BB", "name": "Fone", "kind": "fone", "battery": 5 }] });
        let disk = json!({ "freePct": 9, "totalBytes": 100 });
        let a = alerts(&net, &bt, &disk);
        let ids: Vec<_> = a.iter().map(|x| x["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["net", "pad:AA", "disk"], "fone com bateria baixa não é alerta; 20% é alerta");
    }

    #[test]
    fn sem_gerenciador_de_rede_nao_alerta() {
        let net = net_part(&json!({ "available": false }));
        assert!(alerts(&net, &json!({}), &json!({ "freePct": 50, "totalBytes": 1 })).is_empty());
        let n = net_part(&json!({ "available": true, "online": false, "main": null }));
        assert_eq!(n["kind"], "none");
        assert_eq!(alerts(&n, &json!({}), &json!({}))[0]["text"], "Sem conexão com a rede");
    }

    #[test]
    fn disco_lido() {
        let d = disk_part(std::path::Path::new("/tmp"));
        assert!(d["totalBytes"].as_u64().unwrap() > 0);
    }
}
