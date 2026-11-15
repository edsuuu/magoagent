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
fn the_arms_go_up_at_the_end_of_a_turn_and_come_down_at_rest() {
    let mut motion = MagoMotion::default();
    let time = run(&mut motion, MagoState::Idle, 0.0, 0.5);
    assert!(motion.hands.value < 0.01);
    let time = run(&mut motion, MagoState::Done, time, 1.0);
    assert!(motion.hands.value > 0.95, "{}", motion.hands.value);
    run(&mut motion, MagoState::Idle, time, 1.0);
    assert!(motion.hands.value < 0.02, "arms come down");
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
