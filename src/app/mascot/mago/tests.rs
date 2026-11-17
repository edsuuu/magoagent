use super::motion::ParticleKind;
use super::*;

fn run(motion: &mut MagoMotion, state: MagoState, from: f64, seconds: f64) -> f64 {
    let mut time = from;
    while time < from + seconds {
        motion.advance(state, time, None);
        time += 1.0 / 60.0;
    }
    time
}

#[test]
fn agent_states_become_expressions() {
    let now = 1000.0;
    let state = |agent| MagoState::from_agent(agent, now);
    assert_eq!(state(AgentState::Working(990.0)), MagoState::Working);
    assert_eq!(state(AgentState::Waiting), MagoState::NeedsYou);
    assert_eq!(state(AgentState::Idle(Some(998.0))), MagoState::Done, "a turn that just ended");
    assert_eq!(state(AgentState::Idle(Some(900.0))), MagoState::Idle);
    assert_eq!(state(AgentState::Idle(Some(100.0))), MagoState::Sleeping, "long still");
    assert_eq!(state(AgentState::Idle(None)), MagoState::Idle);
    assert_eq!(MagoState::from_voice(AvatarState::Thinking), MagoState::Working);
    assert_eq!(MagoState::from_voice(AvatarState::Muted), MagoState::Sleeping);
    assert_eq!(Look::of(MagoState::NeedsYou).badge, Some('!'));
    assert_eq!(Look::of(MagoState::Asking).badge, Some('?'));
    assert_eq!(Look::of(MagoState::Error).eye, EyeKind::Cross);
    assert_eq!(Look::of(MagoState::Sleeping).eye, EyeKind::Sleepy);
    assert!(Look::of(MagoState::Done).mouth_curve > Look::of(MagoState::Idle).mouth_curve);
}

#[test]
fn tweens_reach_their_target() {
    let mut motion = MagoMotion::default();
    let time = run(&mut motion, MagoState::Idle, 0.0, 0.1);
    run(&mut motion, MagoState::Error, time, 2.0);
    let error = Look::of(MagoState::Error);
    assert!((motion.mouth_curve.value - error.mouth_curve).abs() < 0.002);
    assert!((motion.tint_amount.value - error.tint.1).abs() < 0.01);
}

/// The eyes of a wizard at 100, 100 that looked ahead, after `seconds` of looking toward `pointer`.
fn eyes_toward(pointer: Pos2, seconds: f64) -> [motion::Eye; 2] {
    let mut motion = MagoMotion::default();
    motion.advance(MagoState::Idle, 0.0, Some(Vec2::ZERO));
    let mut time = 1.0 / 60.0;
    while time < seconds {
        motion.advance(MagoState::Idle, time, Some(gaze_toward(Pos2::new(100.0, 100.0), pointer)));
        time += 1.0 / 60.0;
    }
    motion.eyes().map(|eye| eye.expect("both eyes show"))
}

#[test]
fn the_eyes_follow_a_pointer_anywhere_and_far_to_a_side_they_go_well_to_that_side() {
    use motion::{BODY_CENTER, BODY_HALF};
    let middle = |eyes: [motion::Eye; 2]| (eyes[0].at + eyes[1].at) / 2.0;
    // on the wizard: straight ahead, one eye to each side
    let ahead = eyes_toward(Pos2::new(100.0, 100.0), 3.0);
    assert!(middle(ahead).x.abs() < 0.001 && ahead[0].at.x < -0.1 && ahead[1].at.x > 0.1, "{ahead:?}");
    // 1000 points to the right: more than a quarter of the body's half width that way, the near eye narrower
    let right = eyes_toward(Pos2::new(1100.0, 100.0), 3.0);
    assert!(middle(right).x > 0.25 * BODY_HALF.0, "{right:?}");
    assert!(right[1].flat.x < right[0].flat.x && right[1].flat.x < 0.7, "the eye near the edge is seen from the side: {right:?}");
    let left = eyes_toward(Pos2::new(-900.0, 100.0), 3.0);
    assert!((middle(left).x + middle(right).x).abs() < 0.001, "the same to the left");
    // above and below
    let (up, down) = (eyes_toward(Pos2::new(100.0, -900.0), 3.0), eyes_toward(Pos2::new(100.0, 1100.0), 3.0));
    assert!(middle(up).y < middle(ahead).y - 0.08 && middle(down).y > middle(ahead).y + 0.08, "{up:?} {down:?}");
    // it turns over a moment, not at once
    let soon = eyes_toward(Pos2::new(1100.0, 100.0), 0.1);
    assert!(middle(soon).x > 0.01 && middle(soon).x < middle(right).x * 0.7, "{soon:?}");
    // wherever the pointer is, the whole eye (the widest one) stays on the squircle, a superellipse of power 4
    for pointer in [(5000.0, 100.0), (-5000.0, -5000.0), (5000.0, 5000.0), (100.0, 9000.0), (-3000.0, 4000.0)] {
        for eye in eyes_toward(Pos2::new(pointer.0, pointer.1), 3.0) {
            let radius = (BODY_HALF.0 * BODY_HALF.1).sqrt();
            let (wide, tall) = (radius * 0.25 * 1.16 / 2.0 * eye.flat.x, radius * 0.27 * 1.12 / 2.0 * eye.flat.y);
            let corner = ((eye.at.x.abs() + wide) / BODY_HALF.0).powi(4) + (((eye.at.y - BODY_CENTER).abs() + tall) / BODY_HALF.1).powi(4);
            assert!(corner < 1.0, "{pointer:?}: {eye:?}");
        }
    }
}

#[test]
fn a_mini_one_glances_around_by_itself_at_a_new_place_every_moment() {
    let mut motion = MagoMotion::seeded(7);
    let mut places = Vec::new();
    let mut time = 0.0;
    while time < 20.0 {
        let glance = motion.glance(time);
        assert!(glance.x.abs() <= 0.88 && (-0.45..=0.55).contains(&glance.y), "{glance:?}");
        if places.last() != Some(&glance) {
            places.push(glance);
        }
        time += 1.0 / 30.0;
    }
    // a new place every 0.5 to 2 s
    assert!((10..=40).contains(&places.len()), "{}", places.len());
}

#[test]
fn finishing_throws_sparks_that_die_out() {
    let mut motion = MagoMotion::default();
    let time = run(&mut motion, MagoState::Working, 0.0, 0.2);
    let time = run(&mut motion, MagoState::Done, time, 0.3);
    assert!(motion.particles.iter().filter(|particle| particle.kind == ParticleKind::Spark).count() >= 10, "a burst");
    // it keeps being Done but the burst stops, and every spark ends
    run(&mut motion, MagoState::Idle, time, 3.0);
    assert!(motion.particles.is_empty(), "{} left", motion.particles.len());
}

#[test]
fn sleeping_breathes_zzz_and_a_hop_lands() {
    let mut motion = MagoMotion::default();
    let time = run(&mut motion, MagoState::Sleeping, 0.0, 3.0);
    assert!(motion.particles.iter().any(|particle| particle.kind == ParticleKind::Zzz));
    let time = run(&mut motion, MagoState::Done, time, 0.1);
    assert!(motion.hop > 0.0, "the hop of the end of a turn");
    run(&mut motion, MagoState::Idle, time, 1.5);
    assert_eq!(motion.hop, 0.0, "back on the ground");
}

#[test]
fn clicks_annoy_then_make_it_dizzy() {
    let mut motion = MagoMotion::default();
    let mut time = run(&mut motion, MagoState::Idle, 0.0, 0.2);
    motion.poke(time);
    time = run(&mut motion, MagoState::Idle, time, 0.05);
    assert_eq!(motion.look.eye, EyeKind::Focus);
    for _ in 0..3 {
        time += 0.2;
        motion.poke(time);
    }
    run(&mut motion, MagoState::Idle, time, 0.05);
    assert_eq!(motion.look.eye, EyeKind::Spiral);
    let time = run(&mut motion, MagoState::Idle, time, 3.0);
    motion.advance(MagoState::Idle, time, None);
    assert_eq!(motion.look.eye, EyeKind::Round, "it recovers");
}

#[test]
fn it_paints_at_every_size_without_panicking() {
    let ctx = Context::default();
    let mut motion = MagoMotion::default();
    let mut time = 0.0;
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        for state in [
            MagoState::Idle,
            MagoState::Working,
            MagoState::NeedsYou,
            MagoState::Asking,
            MagoState::Done,
            MagoState::Error,
            MagoState::Sleeping,
            MagoState::Speaking,
            MagoState::Waving,
        ] {
            for size in [24.0, 60.0, 160.0] {
                time += 0.1;
                paint_mago(ui.painter(), Pos2::new(100.0, 100.0), size, state, &mut motion, time, Some(Pos2::new(500.0, 20.0)));
            }
        }
    });
}

#[test]
fn the_arms_are_down_except_for_the_greeting_wave() {
    let mut motion = MagoMotion::default();
    let mut time = run(&mut motion, MagoState::Idle, 0.0, 0.5);
    for state in [MagoState::Working, MagoState::NeedsYou, MagoState::Done, MagoState::Error] {
        time = run(&mut motion, state, time, 0.6);
        assert!(motion.hands.value < 0.01, "{state:?}: {}", motion.hands.value);
    }
    time = run(&mut motion, MagoState::Waving, time, 0.26);
    assert!(motion.hands.value < 0.1, "they wait 250 ms before waving: {}", motion.hands.value);
    time = run(&mut motion, MagoState::Waving, time, 0.32);
    assert!(motion.hands.value > 0.85, "they rise over 280 ms: {}", motion.hands.value);
    time = run(&mut motion, MagoState::Waving, time, 1.0);
    assert!(motion.hands.value > 0.9, "they stay up through the greeting: {}", motion.hands.value);
    time = run(&mut motion, MagoState::Waving, time, 0.22);
    assert!(motion.hands.value < 0.15, "they return down over 200 ms: {}", motion.hands.value);
    run(&mut motion, MagoState::Idle, time, 0.5);
    assert!(motion.hands.value < 0.01, "they stay down at rest");
}

#[test]
fn the_right_hand_waves_pointing_up_and_out_from_its_pivot() {
    let mut motion = MagoMotion::default();
    let mut time = run(&mut motion, MagoState::Idle, 0.0, 0.3);
    for _ in 0..180 {
        motion.advance(MagoState::NeedsYou, time, None);
        time += 1.0 / 60.0;
        let (near, far) = motion.hand_ends(1);
        if motion.hands.value > 0.9 {
            // egui's y grows down: up is a smaller y, out to the right a bigger x
            assert!(far.x > near.x && far.y < near.y, "{near:?} {far:?}");
            let degrees = motion.hand_angle[1].to_degrees();
            assert!((24.0..=66.0).contains(&degrees), "{degrees}");
        }
    }
    let (left_near, left_far) = motion.hand_ends(0);
    assert!((left_far - left_near).length() < 0.001, "the idle left hand stays a ball");
    assert!(left_near.x < 0.0);
}

#[test]
fn a_gaze_that_is_not_a_number_does_not_blind_it() {
    let mut motion = MagoMotion::default();
    let mut time = run(&mut motion, MagoState::Idle, 0.0, 0.5);
    motion.advance(MagoState::Idle, time, Some(Vec2::new(f32::NAN, f32::INFINITY)));
    time = run(&mut motion, MagoState::Idle, time, 0.5);
    let [left, right] = motion.eyes().map(|eye| eye.expect("both eyes show").at);
    assert!(left.x.is_finite() && left.y.is_finite() && right.x.is_finite(), "{left:?}");
    // and a wizard of no size, which divides the pointer's distance by nothing
    let ctx = Context::default();
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        paint_mago_toward(ui.painter(), "sem-tamanho", Pos2::new(50.0, 50.0), 0.0, MagoState::Idle, 1.0, Some(Pos2::new(80.0, 10.0)));
        paint_mago_toward(ui.painter(), "sem-tamanho", Pos2::new(50.0, 50.0), 62.0, MagoState::Idle, 1.1, Some(Pos2::new(80.0, 10.0)));
    });
    MOTIONS.with(|motions| {
        let motions = motions.borrow();
        let motion = &motions[&motion_key("sem-tamanho")];
        assert!(motion.look_x.value.is_finite() && motion.look_y.value.is_finite());
    });
    let _ = time;
}

mod dancing {
    use super::*;
    use crate::app::integrations::spotify::groove::{Dance, Groove};
    use dance::{Place, pose};
    use draw::dance_expression;

    const GREEN: Color32 = Color32::from_rgb(0x1e, 0xd7, 0x60);

    fn groove(dance: Dance, beat: f64, energy: f32) -> Groove {
        Groove { bpm: 120.0, phase: beat.fract() as f32, energy, bass: 0.3, bands: None, dance, beat, since: 0.0 }
    }

    fn close(one: f32, other: f32) -> bool {
        (one - other).abs() < 0.01
    }

    /// The shapes a dancer leaves, painted alone.
    fn shapes(groove: &Groove, size: f32) -> usize {
        let ctx = Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            paint_mago_dancing(ui.painter(), &format!("luna-{:?}-{size}", groove.dance), Pos2::new(50.0, 50.0), size, GREEN, groove, 1.0);
        });
        output.textures_delta.clear();
        output.shapes.len()
    }

    #[test]
    fn only_palmas_maos_pro_alto_onda_and_giro_have_arms() {
        let with_arms = [Dance::Clap, Dance::Pump, Dance::Wave, Dance::Spin];
        let without = shapes(&groove(Dance::Sway, 0.3, 0.7), 22.0);
        for dance in Dance::ALL {
            let arms = with_arms.contains(&dance);
            assert_eq!(dance.arms(), arms, "{dance:?}");
            assert_eq!(pose(&groove(dance, 0.3, 0.7)).arms.is_some(), arms, "{dance:?}");
            // two tubes more on the screen, at the island's sizes too
            for size in [16.0, 22.0, 48.0] {
                assert_eq!(shapes(&groove(dance, 0.3, 0.7), size), without + if arms { 2 } else { 0 }, "{dance:?} at {size}");
            }
        }
    }

    #[test]
    fn the_poses_are_the_previews_keyframes_and_the_hops_land_on_the_beat() {
        // the sway: two beats at the clock's 76 BPM (the preview's 1.58 s), -7° on the beat and 7° a beat later
        let sway = |beat| pose(&Groove { bpm: 76.0, ..groove(Dance::Sway, beat, 0.5) }).body;
        assert!(close(sway(0.0).turn, -7.0) && close(sway(1.0).turn, 7.0) && close(sway(2.0).turn, -7.0));
        // the step: to the left on one beat, up between, to the right on the next
        let step = |beat| pose(&Groove { bpm: 100.0, ..groove(Dance::Step, beat, 0.5) }).body;
        assert!(close(step(0.0).shift.x, -4.0) && close(step(0.5).shift.y, -3.0) && close(step(1.0).shift.x, 4.0));
        // the hop lands on the beat, squashed (1.05, .95), and is 6 up at 40% of it
        let hop = |beat| pose(&groove(Dance::Pump, beat, 0.5)).body;
        assert!(close(hop(3.0).shift.y, 0.0) && close(hop(3.0).scale.x, 1.05) && close(hop(3.0).scale.y, 0.95));
        assert!(close(hop(3.4).shift.y, -6.0) && close(hop(3.4).scale.y, 1.04));
        // the nod is deepest on the beat, 13° forward
        assert!(close(pose(&groove(Dance::Bang, 5.0, 0.5)).body.turn, 13.0));
        assert!(close(pose(&groove(Dance::Bang, 5.55, 0.5)).body.turn, 0.0));
        // asleep it only breathes, wider and lower
        let nap = pose(&Groove { bpm: 40.0, ..groove(Dance::Nap, 1.0, 0.0) }).body;
        assert!(close(nap.scale.x, 1.03) && close(nap.scale.y, 0.97) && nap.shift == Vec2::ZERO && nap.turn == 0.0);
    }

    #[test]
    fn the_arms_clap_above_the_head_wider_the_louder_and_the_spin_happens_once() {
        // Palmas at 108 BPM (a beat each): the two tubes meet over the head halfway through the beat, when it is loud
        let clap = |energy| pose(&Groove { bpm: 108.0, ..groove(Dance::Clap, 2.5, energy) }).arms.unwrap();
        let [left, right] = clap(1.0);
        assert!(close(left.shift.x, 17.6) && close(left.shift.y, -5.0) && close(left.turn, 0.0), "{left:?}");
        assert!(close(right.shift.x, 22.4) && close(right.shift.y, -5.0) && close(right.turn, 0.0), "{right:?}");
        let [quiet, _] = clap(0.0);
        assert!(quiet.shift.x > 6.0 && quiet.shift.x < 17.0, "quieter, they do not reach: {quiet:?}");
        // on the beat they are open, as the preview's 0%
        let [open, _] = pose(&Groove { bpm: 108.0, ..groove(Dance::Clap, 3.0, 1.0) }).arms.unwrap();
        assert!(close(open.shift.x, 6.0) && close(open.turn, -28.0));
        // the spin: from its beat (4), four beats at 120; mirrored at 45%, the arms straight out, landing at 75% (beat 7)
        let spin = |beat| pose(&Groove { since: 4.0, ..groove(Dance::Spin, beat, 0.2) });
        assert_eq!(spin(4.0).body, Place { shift: Vec2::ZERO, turn: 0.0, scale: Vec2::splat(1.0) });
        assert!(close(spin(5.8).body.scale.x, -1.0) && close(spin(5.8).body.shift.y, -5.0));
        let [left, right] = spin(6.0).arms.unwrap();
        assert!(close(left.turn, 90.0) && close(right.turn, -90.0), "whole even when quiet: {left:?}");
        assert!(close(spin(7.0).body.shift.y, 0.0) && close(spin(7.0).body.scale.x, 1.0));
        assert_eq!(spin(30.0).body, spin(8.0).body, "once: it stays at rest after");
    }

    #[test]
    fn a_new_dance_takes_over_in_a_moment_and_her_eyes_close_to_nap() {
        let key = motion_key("luna-mistura");
        let (sway, clap) = (pose(&groove(Dance::Sway, 0.0, 1.0)), pose(&groove(Dance::Clap, 0.0, 1.0)));
        dance::blended(key, Dance::Sway, sway, 10.0);
        let start = dance::blended(key, Dance::Clap, clap, 10.1);
        assert!(start.body == sway.body && start.arms_alpha == 0.0, "it starts from the last pose: {start:?}");
        let midway = dance::blended(key, Dance::Clap, clap, 10.2);
        assert!(midway.arms_alpha > 0.0 && midway.arms_alpha < 1.0 && midway.body != clap.body, "{midway:?}");
        assert_eq!(dance::blended(key, Dance::Clap, clap, 10.4), clap);

        let ctx = Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            for (id, dance) in [("luna-cochila", Dance::Nap), ("luna-palmas", Dance::Clap)] {
                for time in [1.0, 1.5] {
                    paint_mago_dancing(ui.painter(), id, Pos2::new(40.0, 40.0), 22.0, GREEN, &groove(dance, time, 0.5), time);
                }
            }
            // no size, or nothing that is a number: nothing drawn, nothing broken
            paint_mago_dancing(ui.painter(), "luna-nada", Pos2::new(40.0, 40.0), 0.0, GREEN, &groove(Dance::Clap, 1.0, 0.5), 1.0);
            paint_mago_dancing(ui.painter(), "luna-nada", Pos2::new(40.0, 40.0), 18.0, GREEN, &groove(Dance::Clap, f64::NAN, f32::NAN), 1.0);
        });
        output.textures_delta.clear();
        MOTIONS.with(|motions| {
            let motions = motions.borrow();
            assert_eq!(motions[&motion_key("luna-cochila")].look.eye, EyeKind::Sleepy, "asleep: the eyes closed");
            assert_eq!(motions[&motion_key("luna-palmas")].look.eye, EyeKind::Round, "dancing: the eyes as always");
            assert!(!motions.contains_key(&motion_key("luna-nada")));
        });
    }

    #[test]
    fn low_energy_gives_happy_eyes() {
        let groove = groove(Dance::Clap, 0.5, 0.1);
        let (eye, _curve, _open) = dance_expression(&groove);
        assert_eq!(eye, EyeKind::Happy, "low energy while dancing should give happy eyes");
    }

    #[test]
    fn high_energy_gives_wide_eyes() {
        let groove = groove(Dance::Clap, 0.5, 0.9);
        let (eye, _curve, _open) = dance_expression(&groove);
        assert_eq!(eye, EyeKind::Wide, "high energy peak should give wide eyes");
    }

    #[test]
    fn normal_energy_gives_round_eyes() {
        let groove = groove(Dance::Sway, 0.5, 0.5);
        let (eye, _curve, _open) = dance_expression(&groove);
        assert_eq!(eye, EyeKind::Round, "normal energy should give round eyes");
    }

    #[test]
    fn high_bass_near_beat_gives_open_smile() {
        // bass > 0.7 and near phase 0 (phase = 0.02 → near_beat = 0.02)
        let groove = Groove {
            bpm: 120.0, phase: 0.02, energy: 0.5, bass: 0.85, bands: None, dance: Dance::Clap, beat: 10.02, since: 0.0,
        };
        let (_eye, curve, open) = dance_expression(&groove);
        assert!(curve > 0.05, "high bass near beat should give a wider smile curve: {}", curve);
        assert!(open > 0.02, "high bass very near beat should open mouth: {}", open);
    }

    #[test]
    fn low_bass_gives_neutral_mouth() {
        let groove = groove(Dance::Sway, 0.5, 0.5);
        let (_eye, curve, open) = dance_expression(&groove);
        assert!(close(curve, 0.03), "low bass should give default mouth curve: {}", curve);
        assert!(close(open, 0.0), "low bass should give neutral (closed) mouth: {}", open);
    }

    #[test]
    fn high_bass_not_near_beat_gives_smile_no_open() {
        // phase = 0.5 → near_beat = 0.5 (not near beat)
        let groove = Groove {
            bpm: 120.0, phase: 0.5, energy: 0.5, bass: 0.9, bands: None, dance: Dance::Clap, beat: 10.5, since: 0.0,
        };
        let (_eye, curve, open) = dance_expression(&groove);
        assert!(close(curve, 0.03), "high bass far from beat should keep default curve");
        assert!(close(open, 0.0), "high bass far from beat should not open mouth");
    }

    #[test]
    fn nap_gives_sleepy_eyes() {
        let groove = Groove {
            bpm: 40.0, phase: 0.5, energy: 0.1, bass: 0.1, bands: None, dance: Dance::Nap, beat: 1.0, since: 0.0,
        };
        let (eye, _curve, _open) = dance_expression(&groove);
        assert_eq!(eye, EyeKind::Sleepy, "nap should give sleepy eyes regardless of energy");
    }
}

/// Speaking, the mouth sits under the eyes and between them, wherever she looks (asked: "a boca da luna nessa intro
/// ficou horrível": a dash fixed in the middle showed under one eye while she looked at her bubble).
#[test]
fn the_mouth_follows_the_eyes() {
    let mut motion = MagoMotion::default();
    for look in [-0.8, 0.0, 0.8] {
        motion.look_x = crate::app::mascot::mago::motion::Tween::new(look);
        let [Some(left), Some(right)] = motion.eyes() else { panic!("both eyes at {look}") };
        let mouth = motion.mouth().expect("a mouth facing ahead");
        let between = (left.at.x + right.at.x) / 2.0;
        assert!((mouth.at.x - between).abs() < 0.03, "{look}: mouth {} eyes {between}", mouth.at.x);
        assert!(mouth.at.y > left.at.y.max(right.at.y) + 0.05, "{look}: under the eyes");
    }
}
