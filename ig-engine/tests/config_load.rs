//! Guard: the shipped config/default.toml must deserialize into EngineConfig.
//! Added in Phase 17.F after the instrument_overrides section gained its first
//! TOML entry — nothing previously exercised the real file in CI, so a typo
//! there would only surface as an engine crash at startup.

use ig_engine::engine::config::EngineConfig;

#[test]
fn default_toml_parses() {
    let config = EngineConfig::load("config/default.toml").expect("config/default.toml must parse");

    // Phase 17.F/17.G values — fail loudly if someone reverts them by accident.
    // Any per-instrument M15 SL/TP override must clear the min_risk_reward
    // gate or every trade on that epic is silently rejected.
    for epic in ["CS.D.EURUSD.CSD.IP", "CS.D.USDJPY.CSD.IP"] {
        let ov = config
            .strategies
            .instrument_overrides
            .get(epic)
            .unwrap_or_else(|| panic!("{epic} instrument override must exist"));
        assert_eq!(ov.m15_atr_sl_multiplier, Some(2.5), "{epic} SL multiplier");
        assert_eq!(ov.m15_atr_tp_multiplier, Some(6.5), "{epic} TP multiplier");
        let rr = ov.m15_atr_tp_multiplier.unwrap() / ov.m15_atr_sl_multiplier.unwrap();
        assert!(
            rr >= config.risk.min_risk_reward,
            "{epic} override R:R {} below min_risk_reward {}",
            rr,
            config.risk.min_risk_reward
        );
    }

    assert_eq!(config.risk.volatile_breakeven_trigger, 0.9);
    // Phase 17.G — same-instrument entry spacing.
    assert_eq!(config.strategies.m15_min_entry_spacing_secs, 2700);
}
