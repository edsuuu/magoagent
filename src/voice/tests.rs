use super::*;

#[test]
fn talking_over_the_agent_breaks_in_and_its_own_echo_does_not() {
    let block = |level: f32, k: usize| -> Vec<f32> { (0..BLOCK).map(|i| level * 1.41 * ((i + k * BLOCK) as f32 * 0.3).sin()).collect() };
    let mut barge = BargeIn::default();
    // the speaker's echo, loud from the very start and rising and falling as it speaks
    for k in 0..150 {
        let level = 0.08 + 0.04 * (k as f32 * 0.4).sin();
        assert!(!barge.feed(&block(level, k)), "the echo broke in at block {k}");
    }
    // someone talking well over it: within a quarter second and a little
    let at = (150..200).find(|&k| barge.feed(&block(0.45, k))).expect("the voice never broke in");
    assert!(at - 150 < 15, "{at}");
    assert_eq!(barge.kept.len(), 15);
}

#[test]
fn speech_is_cut_by_its_level_with_a_lead_in() {
    let mut s = Segmenter::with_pause(0.8);
    let quiet = vec![0.001f32; BLOCK];
    let loud: Vec<f32> = (0..BLOCK).map(|i| if i % 2 == 0 { 0.3 } else { -0.3 }).collect();
    for _ in 0..50 {
        assert_eq!(s.feed(&quiet), None);
    }
    // 1 s of speech, then quiet: the stretch comes after 0.8 s of it, with 0.3 s before
    let mut got = None;
    for _ in 0..50 {
        assert_eq!(s.feed(&loud), None);
    }
    for _ in 0..45 {
        if let Some(samples) = s.feed(&quiet) {
            got = Some(samples);
            break;
        }
    }
    let samples = got.expect("a stretch of speech");
    assert!(samples.len() >= 50 * BLOCK + 15 * BLOCK && samples.len() <= 50 * BLOCK + 15 * BLOCK + 40 * BLOCK, "{}", samples.len());
    // a short click isn't speech
    let mut s = Segmenter::default();
    for _ in 0..50 {
        s.feed(&quiet);
    }
    for _ in 0..5 {
        s.feed(&loud);
    }
    assert!((0..45).all(|_| s.feed(&quiet).is_none()));
    // with the default pause (2 s) a 1 s pause to think is part of what is said
    let mut s = Segmenter::default();
    for _ in 0..50 {
        s.feed(&quiet);
    }
    let mut cut = 0;
    for block in
        std::iter::repeat_n(&loud, 50).chain(std::iter::repeat_n(&quiet, 50)).chain(std::iter::repeat_n(&loud, 50)).chain(std::iter::repeat_n(&quiet, 110))
    {
        if let Some(samples) = s.feed(block) {
            cut += 1;
            assert!(samples.len() >= 150 * BLOCK, "both halves in one: {}", samples.len());
        }
    }
    assert_eq!(cut, 1);
}

#[test]
fn audio_goes_to_mono_16k_and_wav() {
    let stereo: Vec<f32> = (0..96).flat_map(|i| [i as f32 / 96.0, i as f32 / 96.0]).collect();
    let mono = to_16k(&stereo, 2, 48_000);
    assert_eq!(mono.len(), 32);
    assert!((mono[1] - 3.0 / 96.0).abs() < 1e-5, "{}", mono[1]);
    let file = wav(&[0.0, 0.5, -1.0], 16_000);
    assert_eq!(file.len(), 44 + 6);
    assert_eq!(&file[..4], b"RIFF");
    assert_eq!(i16::from_le_bytes([file[46], file[47]]), 16383);
}

#[test]
fn the_person_commands_the_call_by_the_agents_name() {
    assert_eq!(voice_command("Claude, se muta.", "claude"), Some(VoiceCmd::Mute));
    assert_eq!(voice_command("claude mudo", "claude"), Some(VoiceCmd::Mute));
    assert_eq!(voice_command("Claude, desmuta", "claude"), Some(VoiceCmd::Unmute));
    assert_eq!(voice_command("ei Cloud, pode falar", "claude"), Some(VoiceCmd::Speak));
    assert_eq!(voice_command("Claude?", "claude"), Some(VoiceCmd::Speak));
    assert_eq!(voice_command("Claude, roda os testes de novo", "claude"), Some(VoiceCmd::Ask("roda os testes de novo".into())));
    assert_eq!(voice_command("roda os testes, Claude", "claude"), None, "only when called first");
    assert_eq!(voice_command("Codex, se muta", "codex"), Some(VoiceCmd::Mute));
    assert_eq!(yes_or_no("Sim, pode."), Some(true));
    assert_eq!(yes_or_no("Pode sim"), Some(true));
    assert_eq!(yes_or_no("Não, continua."), Some(false));
    assert_eq!(yes_or_no("então a gente vai no mercado"), None);
    assert_eq!(voice_command("Claude, se muta", "codex"), None);
    assert_eq!(
        voice_url("pt_BR-faber-medium").as_deref(),
        Some("https://huggingface.co/rhasspy/piper-voices/resolve/main/pt/pt_BR/faber/medium/pt_BR-faber-medium.onnx")
    );
    assert_eq!(voice_url("pt_BR-dii-ovos").as_deref(), Some("https://huggingface.co/OpenVoiceOS/pipertts_pt-BR_dii/resolve/main/dii_pt-BR.onnx"));
    assert_eq!(voice_url("errado"), None);
    assert!(VOICES.iter().all(|v| voice_url(v).is_some()));
    assert_eq!(voice_name("pt_BR-dii-ovos"), "Dii (feminina)");
    assert_eq!(voice_name("pt_BR-faber-medium"), "Faber (masculina)");
    assert!(VOICES.iter().all(|v| v.starts_with("pt_BR")));
}

#[test]
fn answers_are_said_a_sentence_at_a_time_without_the_markdown() {
    let text = "## Pronto\n\nFiz **duas** coisas. A primeira foi `cargo test`! Veja:\n```sh\ncargo test\n```\n- um item\n- outro item?\nFim";
    assert_eq!(sentences(text), ["Pronto", "Fiz duas coisas.", "A primeira foi cargo test!", "Veja:", "um item", "outro item?", "Fim"]);
}

#[test]
fn the_call_says_the_summary_without_paths_and_points_to_the_screen() {
    let text = "Corrigi o bug do chat. A mudança está em src/app/chat.rs e no voice.rs:42. Os testes passaram.\n\nDetalhes: veja https://x.dev/a.";
    assert_eq!(spoken(text), ["Corrigi o bug do chat.", "Os testes passaram.", "Os detalhes estão na tela."]);
    assert_eq!(spoken("Pronto, tudo certo."), ["Pronto, tudo certo."]);
    assert_eq!(spoken("Feito.\n```rs\nfn a() {}\n```"), ["Feito.", "Os detalhes estão na tela."]);
    assert!(!has_path("Versão 3.5 do Opus, etc.") && has_path("Abra C:\\x") && has_path("veja chat.rs:42."));
}

#[test]
fn the_call_sounds_are_short_quiet_and_the_start_rises() {
    for sound in [CALL_START, CALL_END, CALL_ERROR, CALL_MUTE, CALL_UNMUTE, CALL_WAITING] {
        let samples = sound_samples(sound, 22_050);
        assert!(!samples.is_empty() && sound_length(sound) < 1.0 && samples.iter().all(|s| s.abs() < 0.5));
    }
    // the start's pitch climbs: more zero crossings at its end than at its beginning
    let start = sound_samples(CALL_START, 22_050);
    let crossings = |w: &[f32]| w.windows(2).filter(|p| (p[0] < 0.0) != (p[1] < 0.0)).count();
    assert!(crossings(&start[..2205]) < crossings(&start[4410..6615]));
    assert!(sound_length(CALL_MUTE) < 0.2);
}
