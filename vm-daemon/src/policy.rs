// policy.rs — Krypt Policy Engine
//
// Definiert welche VMs miteinander kommunizieren dürfen.
// Konfiguriert via /etc/krypt/daemon.toml [policy]-Sektion.
//
// Trust-Level:
//   0 = red     (untrusted: browser, unbekannte Geräte)
//   1 = orange  (low-trust: gaming, social)
//   2 = yellow  (medium: work-untrusted)
//   3 = green   (trusted: work, personal)
//   4 = black   (vault: crypto keys, passwords)
#![allow(dead_code)]

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrustLevel {
    Red    = 0,
    Orange = 1,
    Yellow = 2,
    Green  = 3,
    Black  = 4,
}

impl TrustLevel {
    pub fn to_str(&self) -> &'static str {
        match self {
            TrustLevel::Red    => "red",
            TrustLevel::Orange => "orange",
            TrustLevel::Yellow => "yellow",
            TrustLevel::Green  => "green",
            TrustLevel::Black  => "black",
        }
    }
}

#[derive(Debug)]
pub struct PolicyRule {
    pub source_vm:  String,
    pub target_vm:  String,
    pub action:     PolicyAction,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PolicyAction {
    Allow,
    Deny,
    AskUser,
}

pub struct PolicyEngine {
    rules:      Vec<PolicyRule>,
    trust_map:  HashMap<String, TrustLevel>,
}

impl PolicyEngine {
    pub fn new() -> Self {
        Self {
            rules:     Vec::new(),
            trust_map: HashMap::new(),
        }
    }

    pub fn add_rule(&mut self, rule: PolicyRule) {
        self.rules.push(rule);
    }

    pub fn set_trust(&mut self, vm: String, level: TrustLevel) {
        self.trust_map.insert(vm, level);
    }

    /// Gibt den Trust-Level einer VM zurück. Unbekannte VMs → Red (restriktivster Default).
    pub fn get_trust(&self, vm: &str) -> &TrustLevel {
        self.trust_map.get(vm).unwrap_or(&TrustLevel::Red)
    }

    /// Prüft ob source_vm eine Aktion auf target_vm durchführen darf.
    pub fn check(&self, source: &str, target: &str) -> &PolicyAction {
        // Explizite Regel hat Vorrang
        for rule in &self.rules {
            if rule.source_vm == source && rule.target_vm == target {
                return &rule.action;
            }
        }

        // Fallback: Trust-Level-Vergleich
        // Höheres Trust-Level darf nicht von niedrigerem kontaktiert werden
        let src_trust = self.trust_map.get(source).unwrap_or(&TrustLevel::Red);
        let tgt_trust = self.trust_map.get(target).unwrap_or(&TrustLevel::Red);

        if src_trust >= tgt_trust {
            &PolicyAction::AskUser
        } else {
            &PolicyAction::Deny
        }
    }

    /// Lädt Trust-Level und Policy-Regeln aus einer bereits geparsten KryptConfig.
    pub fn load_from_config(&mut self, cfg: &crate::config::KryptConfig) {
        for vm in &cfg.vms {
            self.set_trust(vm.name.clone(), map_trust(vm.trust_level));
        }
        for entry in &cfg.policy {
            self.add_rule(PolicyRule {
                source_vm: entry.source.clone(),
                target_vm: entry.target.clone(),
                action:    map_action(entry.action),
            });
        }
    }

    /// Lädt Trust-Level und Policy-Regeln direkt aus einer TOML-Datei.
    pub fn load_from_toml(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let cfg = crate::config::KryptConfig::load(std::path::Path::new(path))?;
        self.load_from_config(&cfg);
        Ok(())
    }
}

fn map_trust(t: crate::config::TrustLevel) -> TrustLevel {
    match t {
        crate::config::TrustLevel::Red    => TrustLevel::Red,
        crate::config::TrustLevel::Orange => TrustLevel::Orange,
        crate::config::TrustLevel::Yellow => TrustLevel::Yellow,
        crate::config::TrustLevel::Green  => TrustLevel::Green,
        crate::config::TrustLevel::Black  => TrustLevel::Black,
    }
}

fn map_action(a: crate::config::PolicyAction) -> PolicyAction {
    match a {
        crate::config::PolicyAction::Allow => PolicyAction::Allow,
        crate::config::PolicyAction::Deny  => PolicyAction::Deny,
        crate::config::PolicyAction::Ask   => PolicyAction::AskUser,
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Basis-Engine mit fünf VMs über alle Trust-Level.
    fn engine_with_trust() -> PolicyEngine {
        let mut e = PolicyEngine::new();
        e.set_trust("vault".into(),    TrustLevel::Black);
        e.set_trust("work".into(),     TrustLevel::Green);
        e.set_trust("personal".into(), TrustLevel::Green);
        e.set_trust("social".into(),   TrustLevel::Orange);
        e.set_trust("browser".into(),  TrustLevel::Red);
        e
    }

    // --- Explizite Regeln ---

    #[test]
    fn explicit_allow_rule() {
        let mut e = engine_with_trust();
        e.add_rule(PolicyRule { source_vm: "work".into(), target_vm: "personal".into(), action: PolicyAction::Allow });
        assert_eq!(e.check("work", "personal"), &PolicyAction::Allow);
    }

    #[test]
    fn explicit_deny_rule() {
        let mut e = engine_with_trust();
        e.add_rule(PolicyRule { source_vm: "browser".into(), target_vm: "vault".into(), action: PolicyAction::Deny });
        assert_eq!(e.check("browser", "vault"), &PolicyAction::Deny);
    }

    #[test]
    fn explicit_askuser_rule() {
        let mut e = engine_with_trust();
        e.add_rule(PolicyRule { source_vm: "work".into(), target_vm: "vault".into(), action: PolicyAction::AskUser });
        assert_eq!(e.check("work", "vault"), &PolicyAction::AskUser);
    }

    #[test]
    fn first_rule_wins_over_later_duplicate() {
        let mut e = engine_with_trust();
        e.add_rule(PolicyRule { source_vm: "work".into(), target_vm: "browser".into(), action: PolicyAction::Allow });
        e.add_rule(PolicyRule { source_vm: "work".into(), target_vm: "browser".into(), action: PolicyAction::Deny });
        assert_eq!(e.check("work", "browser"), &PolicyAction::Allow);
    }

    #[test]
    fn explicit_rule_overrides_trust_level() {
        // Red → Black würde ohne Regel → Deny. Explizite Allow-Regel überschreibt das.
        let mut e = engine_with_trust();
        e.add_rule(PolicyRule { source_vm: "browser".into(), target_vm: "vault".into(), action: PolicyAction::Allow });
        assert_eq!(e.check("browser", "vault"), &PolicyAction::Allow);
    }

    // --- Trust-Level-Fallback ---

    #[test]
    fn higher_trust_to_lower_trust_askuser() {
        // Green (3) → Red (0): src >= tgt → AskUser
        let e = engine_with_trust();
        assert_eq!(e.check("work", "browser"), &PolicyAction::AskUser);
    }

    #[test]
    fn lower_trust_to_higher_trust_denied() {
        // Red (0) → Green (3): src < tgt → Deny
        let e = engine_with_trust();
        assert_eq!(e.check("browser", "work"), &PolicyAction::Deny);
    }

    #[test]
    fn same_trust_level_askuser() {
        // Green == Green → AskUser
        let e = engine_with_trust();
        assert_eq!(e.check("work", "personal"), &PolicyAction::AskUser);
    }

    #[test]
    fn red_cannot_reach_vault() {
        // Red (0) → Black (4): 0 < 4 → Deny
        let e = engine_with_trust();
        assert_eq!(e.check("browser", "vault"), &PolicyAction::Deny);
    }

    #[test]
    fn orange_cannot_reach_green() {
        // Orange (1) → Green (3): 1 < 3 → Deny
        let e = engine_with_trust();
        assert_eq!(e.check("social", "work"), &PolicyAction::Deny);
    }

    #[test]
    fn black_can_reach_red_askuser() {
        // Black (4) → Red (0): 4 >= 0 → AskUser
        let e = engine_with_trust();
        assert_eq!(e.check("vault", "browser"), &PolicyAction::AskUser);
    }

    // --- Unbekannte VMs ---

    #[test]
    fn unknown_vms_both_default_to_red() {
        // Beide unbekannt → Red (0) >= Red (0) → AskUser
        let e = PolicyEngine::new();
        assert_eq!(e.check("ghost-a", "ghost-b"), &PolicyAction::AskUser);
    }

    #[test]
    fn unknown_source_against_known_green_denied() {
        // Unbekannt → Red (0), Ziel Green (3): 0 < 3 → Deny
        let e = engine_with_trust();
        assert_eq!(e.check("ghost", "work"), &PolicyAction::Deny);
    }

    #[test]
    fn known_green_against_unknown_target_askuser() {
        // Green (3) vs. unbekannt → Red (0): 3 >= 0 → AskUser
        let e = engine_with_trust();
        assert_eq!(e.check("work", "ghost"), &PolicyAction::AskUser);
    }
}
