//! system/sysd/nexus-slim (sistema leve): aplica, é idempotente, respeita impressora configurada e desfaz tudo.
use std::path::Path;
use std::process::Command;

fn script() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../system/sysd/nexus-slim")
}

fn run(root: &Path, bin: &Path, arg: &str) -> (bool, String) {
    let o = Command::new("sh")
        .arg(script())
        .arg(arg)
        .env("NEXUS_ROOT", root)
        .env("NEXUS_SYSTEMCTL", bin.join("systemctl"))
        .env("NEXUS_SYSCTL", "true")
        .env("NEXUS_FORCE_ZRAM", "1")
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .output()
        .unwrap();
    (o.status.success(), String::from_utf8_lossy(&o.stdout).into_owned())
}

#[test]
fn slim_apply_and_revert() {
    let d = tempfile::tempdir().unwrap();
    let (root, bin) = (d.path().join("root"), d.path().join("bin"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&bin).unwrap();
    let log = d.path().join("calls");
    // systemctl falso: cups e avahi habilitados, colord desabilitado; indexador do usuário habilitado
    std::fs::write(
        bin.join("systemctl"),
        format!(
            r#"#!/bin/sh
echo "$@" >> {log}
case "$*" in
  "is-enabled cups.service"|"is-enabled avahi-daemon.service"|"is-enabled ModemManager.service") echo enabled ;;
  "is-enabled "*) echo disabled ;;
  "--global is-enabled localsearch-3.service") echo static ;;
  "--global is-enabled "*) echo not-found ;;
esac
exit 0
"#,
            log = log.display()
        ),
    )
    .unwrap();
    // lpstat falso: impressora configurada → o cups fica
    std::fs::write(bin.join("lpstat"), "#!/bin/sh\necho 'device for HP: usb://HP'\n").unwrap();
    for f in ["systemctl", "lpstat"] {
        Command::new("chmod").arg("+x").arg(bin.join(f)).status().unwrap();
    }
    let (ok, out) = run(&root, &bin, "apply");
    assert!(ok, "{out}");
    let calls = std::fs::read_to_string(&log).unwrap();
    assert!(calls.contains("disable --now avahi-daemon.service"), "{calls}");
    assert!(calls.contains("disable --now ModemManager.service"), "sem modem: desliga\n{calls}");
    assert!(!calls.contains("disable --now cups.service"), "impressora configurada: cups fica\n{calls}");
    assert!(!calls.contains("disable --now colord.service"), "já estava desligado: não mexe\n{calls}");
    assert!(calls.contains("--global mask localsearch-3.service"), "{calls}");
    let zram = std::fs::read_to_string(root.join("etc/systemd/zram-generator.conf")).unwrap();
    assert!(zram.contains("zram-size = min(ram / 2, 4096)"));
    assert!(std::fs::read_to_string(root.join("etc/sysctl.d/90-nexus.conf")).unwrap().contains("vm.swappiness = 150"));
    let t = std::fs::read_to_string(root.join("etc/systemd/system/apt-daily-upgrade.timer.d/90-nexus.conf")).unwrap();
    assert!(t.contains("OnCalendar=*-*-* 05:00"));
    assert!(std::fs::read_to_string(root.join("etc/systemd/system/apt-daily.service.d/90-nexus.conf")).unwrap().contains("ExecCondition=/usr/lib/nexus/nexus-slim idle"));

    // de novo: não duplica anotações
    assert!(run(&root, &bin, "apply").0);
    let off = std::fs::read_to_string(root.join("var/lib/nexus/slim/disabled")).unwrap();
    assert_eq!(off.lines().filter(|l| *l == "avahi-daemon.service").count(), 1, "{off}");

    // idle: sem jogo pode; com game.json não
    assert!(run(&root, &bin, "idle").0);
    std::fs::create_dir_all(root.join("run/user/1000/nexus")).unwrap();
    std::fs::write(root.join("run/user/1000/nexus/game.json"), "{}").unwrap();
    assert!(!run(&root, &bin, "idle").0, "jogo aberto: apt espera");

    // arquivo que já existia antes não é apagado no revert
    std::fs::write(root.join("etc/systemd/zram-generator.conf"), "# do usuário\n").unwrap();
    std::fs::write(root.join("var/lib/nexus/slim/files"), std::fs::read_to_string(root.join("var/lib/nexus/slim/files")).unwrap().replace("/etc/systemd/zram-generator.conf\n", "")).unwrap();
    let (ok, out) = run(&root, &bin, "revert");
    assert!(ok, "{out}");
    let calls = std::fs::read_to_string(&log).unwrap();
    assert!(calls.contains("enable --now avahi-daemon.service"), "{calls}");
    assert!(calls.contains("--global unmask localsearch-3.service"), "{calls}");
    assert!(!root.join("etc/sysctl.d/90-nexus.conf").exists());
    assert!(!root.join("etc/systemd/system/apt-daily.timer.d").exists(), "pasta vazia removida");
    assert!(root.join("etc/systemd/zram-generator.conf").exists(), "não era do Nexus");
    assert!(!root.join("var/lib/nexus/slim").exists());
}
