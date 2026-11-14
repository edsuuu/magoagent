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

#[test]
fn the_eyes_stay_inside_the_body_with_the_pointer_far_away() {
    let mut motion = MagoMotion::default();
    let mut time = 0.0;
    for far in [Vec2::new(900.0, -400.0), Vec2::new(-900.0, 900.0), Vec2::new(0.0, 5000.0)] {
        for _ in 0..120 {
            motion.advance(MagoState::Idle, time, Some(far));
            time += 1.0 / 60.0;
        }
        for eye in motion.eye_centers() {
            // the whole eye (about 0.09 across) inside the squircle
            assert!(eye.x.abs() + 0.045 < 0.34 && (eye.y - 0.2).abs() + 0.07 < 0.27, "{eye:?}");
        }
    }
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
        ] {
            for size in [24.0, 60.0, 160.0] {
                time += 0.1;
                paint_mago(ui.painter(), Pos2::new(100.0, 100.0), size, state, &mut motion, time, Some(Pos2::new(500.0, 20.0)));
            }
        }
    });
}

#[test]
fn the_hands_come_out_at_the_end_of_a_turn_and_hide_at_rest() {
    let mut motion = MagoMotion::default();
    let time = run(&mut motion, MagoState::Idle, 0.0, 0.5);
    assert!(motion.hands.value < 0.01);
    let time = run(&mut motion, MagoState::Done, time, 1.0);
    assert!(motion.hands.value > 0.95, "{}", motion.hands.value);
    run(&mut motion, MagoState::Idle, time, 1.0);
    assert!(motion.hands.value < 0.02);
}
