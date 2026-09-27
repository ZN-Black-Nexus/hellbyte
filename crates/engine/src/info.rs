//! Static game definitions: specials, thing kinds, per-kind properties and
//! the state machine table that drives every actor's animation and AI.

use crate::fixed::{Fixed, FRACUNIT};

// ------------------------------------------------------------------ specials

/// Line specials. Prefixes: W = walk over, S = switch (use), G = gun (shoot),
/// 1 = once, R = repeatable. Manual doors (`Door*`) act on the sector behind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum LineSpecial {
    None,
    DoorRepeat,
    DoorRepeatFast,
    DoorStay,
    DoorRedRepeat,
    DoorBlueRepeat,
    DoorYellowRepeat,
    DoorRedStay,
    DoorBlueStay,
    DoorYellowStay,
    W1DoorOpen,
    WrDoorOpen,
    S1DoorOpen,
    SrDoorOpen,
    W1DoorOpenStay,
    S1DoorOpenStay,
    SrDoorOpenStay,
    G1DoorOpenStay,
    W1DoorClose,
    S1DoorClose,
    W1Lift,
    WrLift,
    S1Lift,
    SrLift,
    W1FloorLowerLowest,
    S1FloorLowerLowest,
    W1FloorRaiseNearest,
    S1FloorRaiseNearest,
    W1FloorRaise24,
    S1FloorRaise24,
    W1FloorRaiseHighest,
    W1FloorRaiseCeiling,
    S1FloorRaiseCeiling,
    W1CeilingLowerFloor,
    W1Crusher,
    WrCrusher,
    W1CrusherStop,
    W1Stairs8,
    S1Stairs8,
    W1LightOff,
    W1LightOn,
    W1Teleport,
    WrTeleport,
    S1Exit,
    W1Exit,
    S1SecretExit,
    W1SecretExit,
    ScrollLeft,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SectorSpecial {
    None,
    LightFlicker,
    LightStrobeFast,
    LightStrobeSlow,
    LightGlow,
    Damage5,
    Damage10,
    Damage20,
    Secret,
    DoorClose30,
    ExitDamage,
}

// ------------------------------------------------------------------ mobj flags

pub const MF_SPECIAL: u32 = 1;
pub const MF_SOLID: u32 = 2;
pub const MF_SHOOTABLE: u32 = 4;
pub const MF_NOSECTOR: u32 = 8;
pub const MF_NOBLOCKMAP: u32 = 0x10;
pub const MF_AMBUSH: u32 = 0x20;
pub const MF_JUSTHIT: u32 = 0x40;
pub const MF_JUSTATTACKED: u32 = 0x80;
pub const MF_SPAWNCEILING: u32 = 0x100;
pub const MF_NOGRAVITY: u32 = 0x200;
pub const MF_DROPOFF: u32 = 0x400;
pub const MF_PICKUP: u32 = 0x800;
pub const MF_NOCLIP: u32 = 0x1000;
pub const MF_SLIDE: u32 = 0x2000;
pub const MF_FLOAT: u32 = 0x4000;
pub const MF_TELEPORT: u32 = 0x8000;
pub const MF_MISSILE: u32 = 0x10000;
pub const MF_DROPPED: u32 = 0x20000;
pub const MF_SHADOW: u32 = 0x40000;
pub const MF_NOBLOOD: u32 = 0x80000;
pub const MF_CORPSE: u32 = 0x100000;
pub const MF_INFLOAT: u32 = 0x200000;
pub const MF_COUNTKILL: u32 = 0x400000;
pub const MF_COUNTITEM: u32 = 0x800000;
pub const MF_NOTDMATCH: u32 = 0x1000000;
pub const MF_BOSS: u32 = 0x2000000;
pub const MF_ALWAYSBRIGHT: u32 = 0x4000000;

/// Sprite frame bit: draw at full brightness.
pub const FF_BRIGHT: u8 = 0x80;

// ------------------------------------------------------------------ sprites (procedural drawers)

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Spr {
    None,
    Player,
    Drone,
    Enforcer,
    Heavy,
    Fiend,
    Ripper,
    Gazer,
    Juggernaut,
    FiendBall,
    GazerBall,
    JuggBall,
    Rocket,
    Plasma,
    ArcBall,
    ArcTrace,
    Puff,
    Blood,
    Fog,
    Barrel,
    Lamp,
    Pillar,
    Torch,
    Corpse,
    Clip,
    AmmoBox,
    Shells,
    ShellBox,
    RocketAmmo,
    RocketBox,
    Cell,
    CellPack,
    Backpack,
    Stim,
    Medkit,
    HealthBonus,
    ArmorBonus,
    ArmorGreen,
    ArmorBlue,
    VitalOrb,
    KeyRed,
    KeyBlue,
    KeyYellow,
    PShotgun,
    PChaingun,
    PLauncher,
    PPlasma,
    PArc,
    PDrill,
    Aegis,
    Hazmat,
    SurveyMap,
    Adrenaline,
    // first-person weapons
    WFist,
    WDrill,
    WPistol,
    WShotgun,
    WChaingun,
    WLauncher,
    WPlasma,
    WArc,
}

// ------------------------------------------------------------------ actions

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum A {
    None,
    Look,
    Chase,
    FaceTarget,
    DroneShoot,
    EnforcerShoot,
    HeavyShoot,
    HeavyRefire,
    FiendAttack,
    RipperBite,
    GazerAttack,
    JuggAttack,
    Pain,
    Scream,
    XScream,
    Fall,
    Explode,
    BossDeath,
    PlayerScream,
    // weapons
    WeaponReady,
    Lower,
    Raise,
    ReFire,
    GunFlash,
    Light0,
    Light1,
    Light2,
    FirePunch,
    FireDrill,
    FirePistol,
    FireShotgun,
    FireChaingun,
    FireLauncher,
    FirePlasma,
    ArcCharge,
    FireArc,
    ArcSpray,
}

// ------------------------------------------------------------------ states

pub struct State {
    pub sprite: Spr,
    pub frame: u8,
    pub tics: i16,
    pub action: A,
    pub next: S,
}

const B: u8 = FF_BRIGHT;

macro_rules! states {
    ($($name:ident = $spr:ident $frame:expr, $tics:expr, $act:ident, $next:ident;)*) => {
        /// State identifiers; `STATES[s as usize]` is the state.
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        #[repr(u16)]
        pub enum S { $($name,)* }
        pub static STATES: &[State] = &[
            $(State { sprite: Spr::$spr, frame: $frame, tics: $tics, action: A::$act, next: S::$next },)*
        ];
        impl S {
            pub fn from_u16(v: u16) -> S {
                const ALL: &[S] = &[$(S::$name,)*];
                ALL.get(v as usize).copied().unwrap_or(S::Null)
            }
        }
    };
}

states! {
    Null = None 0, -1, None, Null;
    // ---- player
    PlayStand = Player 0, -1, None, PlayStand;
    PlayRun1 = Player 0, 4, None, PlayRun2;
    PlayRun2 = Player 1, 4, None, PlayRun3;
    PlayRun3 = Player 2, 4, None, PlayRun4;
    PlayRun4 = Player 3, 4, None, PlayRun1;
    PlayAtk1 = Player 4, 12, None, PlayStand;
    PlayAtk2 = Player 5 | B, 6, None, PlayAtk1;
    PlayPain = Player 6, 4, None, PlayPain2;
    PlayPain2 = Player 6, 4, Pain, PlayStand;
    PlayDie1 = Player 7, 10, None, PlayDie2;
    PlayDie2 = Player 8, 10, PlayerScream, PlayDie3;
    PlayDie3 = Player 9, 10, Fall, PlayDie4;
    PlayDie4 = Player 10, 10, None, PlayDie5;
    PlayDie5 = Player 11, -1, None, Null;
    PlayXDie1 = Player 12, 5, None, PlayXDie2;
    PlayXDie2 = Player 13, 5, XScream, PlayXDie3;
    PlayXDie3 = Player 14, 5, Fall, PlayXDie4;
    PlayXDie4 = Player 15, 5, None, PlayXDie5;
    PlayXDie5 = Player 16, -1, None, Null;
    // ---- husk drone (rifle)
    DroneStnd = Drone 0, 10, Look, DroneStnd2;
    DroneStnd2 = Drone 1, 10, Look, DroneStnd;
    DroneRun1 = Drone 0, 4, Chase, DroneRun2;
    DroneRun2 = Drone 0, 4, Chase, DroneRun3;
    DroneRun3 = Drone 1, 4, Chase, DroneRun4;
    DroneRun4 = Drone 1, 4, Chase, DroneRun5;
    DroneRun5 = Drone 2, 4, Chase, DroneRun6;
    DroneRun6 = Drone 2, 4, Chase, DroneRun7;
    DroneRun7 = Drone 3, 4, Chase, DroneRun8;
    DroneRun8 = Drone 3, 4, Chase, DroneRun1;
    DroneAtk1 = Drone 4, 10, FaceTarget, DroneAtk2;
    DroneAtk2 = Drone 5 | B, 8, DroneShoot, DroneAtk3;
    DroneAtk3 = Drone 4, 8, None, DroneRun1;
    DronePain = Drone 6, 3, None, DronePain2;
    DronePain2 = Drone 6, 3, Pain, DroneRun1;
    DroneDie1 = Drone 7, 5, None, DroneDie2;
    DroneDie2 = Drone 8, 5, Scream, DroneDie3;
    DroneDie3 = Drone 9, 5, Fall, DroneDie4;
    DroneDie4 = Drone 10, 5, None, DroneDie5;
    DroneDie5 = Drone 11, -1, None, Null;
    DroneXDie1 = Drone 12, 5, None, DroneXDie2;
    DroneXDie2 = Drone 13, 5, XScream, DroneXDie3;
    DroneXDie3 = Drone 14, 5, Fall, DroneXDie4;
    DroneXDie4 = Drone 15, 5, None, DroneXDie5;
    DroneXDie5 = Drone 16, -1, None, Null;
    // ---- enforcer (shotgun)
    EnfStnd = Enforcer 0, 10, Look, EnfStnd2;
    EnfStnd2 = Enforcer 1, 10, Look, EnfStnd;
    EnfRun1 = Enforcer 0, 3, Chase, EnfRun2;
    EnfRun2 = Enforcer 0, 3, Chase, EnfRun3;
    EnfRun3 = Enforcer 1, 3, Chase, EnfRun4;
    EnfRun4 = Enforcer 1, 3, Chase, EnfRun5;
    EnfRun5 = Enforcer 2, 3, Chase, EnfRun6;
    EnfRun6 = Enforcer 2, 3, Chase, EnfRun7;
    EnfRun7 = Enforcer 3, 3, Chase, EnfRun8;
    EnfRun8 = Enforcer 3, 3, Chase, EnfRun1;
    EnfAtk1 = Enforcer 4, 10, FaceTarget, EnfAtk2;
    EnfAtk2 = Enforcer 5 | B, 10, EnforcerShoot, EnfAtk3;
    EnfAtk3 = Enforcer 4, 10, None, EnfRun1;
    EnfPain = Enforcer 6, 3, None, EnfPain2;
    EnfPain2 = Enforcer 6, 3, Pain, EnfRun1;
    EnfDie1 = Enforcer 7, 5, None, EnfDie2;
    EnfDie2 = Enforcer 8, 5, Scream, EnfDie3;
    EnfDie3 = Enforcer 9, 5, Fall, EnfDie4;
    EnfDie4 = Enforcer 10, 5, None, EnfDie5;
    EnfDie5 = Enforcer 11, -1, None, Null;
    EnfXDie1 = Enforcer 12, 5, None, EnfXDie2;
    EnfXDie2 = Enforcer 13, 5, XScream, EnfXDie3;
    EnfXDie3 = Enforcer 14, 5, Fall, EnfXDie4;
    EnfXDie4 = Enforcer 15, 5, None, EnfXDie5;
    EnfXDie5 = Enforcer 16, -1, None, Null;
    // ---- heavy (rotary gun)
    HvyStnd = Heavy 0, 10, Look, HvyStnd2;
    HvyStnd2 = Heavy 1, 10, Look, HvyStnd;
    HvyRun1 = Heavy 0, 3, Chase, HvyRun2;
    HvyRun2 = Heavy 0, 3, Chase, HvyRun3;
    HvyRun3 = Heavy 1, 3, Chase, HvyRun4;
    HvyRun4 = Heavy 1, 3, Chase, HvyRun5;
    HvyRun5 = Heavy 2, 3, Chase, HvyRun6;
    HvyRun6 = Heavy 2, 3, Chase, HvyRun7;
    HvyRun7 = Heavy 3, 3, Chase, HvyRun8;
    HvyRun8 = Heavy 3, 3, Chase, HvyRun1;
    HvyAtk1 = Heavy 4, 10, FaceTarget, HvyAtk2;
    HvyAtk2 = Heavy 5 | B, 4, HeavyShoot, HvyAtk3;
    HvyAtk3 = Heavy 4 | B, 4, HeavyShoot, HvyAtk4;
    HvyAtk4 = Heavy 5, 1, HeavyRefire, HvyAtk2;
    HvyPain = Heavy 6, 3, None, HvyPain2;
    HvyPain2 = Heavy 6, 3, Pain, HvyRun1;
    HvyDie1 = Heavy 7, 5, None, HvyDie2;
    HvyDie2 = Heavy 8, 5, Scream, HvyDie3;
    HvyDie3 = Heavy 9, 5, Fall, HvyDie4;
    HvyDie4 = Heavy 10, 5, None, HvyDie5;
    HvyDie5 = Heavy 11, -1, None, Null;
    HvyXDie1 = Heavy 12, 5, None, HvyXDie2;
    HvyXDie2 = Heavy 13, 5, XScream, HvyXDie3;
    HvyXDie3 = Heavy 14, 5, Fall, HvyXDie4;
    HvyXDie4 = Heavy 15, 5, None, HvyXDie5;
    HvyXDie5 = Heavy 16, -1, None, Null;
    // ---- fiend (claws + fireballs)
    FndStnd = Fiend 0, 10, Look, FndStnd2;
    FndStnd2 = Fiend 1, 10, Look, FndStnd;
    FndRun1 = Fiend 0, 3, Chase, FndRun2;
    FndRun2 = Fiend 0, 3, Chase, FndRun3;
    FndRun3 = Fiend 1, 3, Chase, FndRun4;
    FndRun4 = Fiend 1, 3, Chase, FndRun5;
    FndRun5 = Fiend 2, 3, Chase, FndRun6;
    FndRun6 = Fiend 2, 3, Chase, FndRun7;
    FndRun7 = Fiend 3, 3, Chase, FndRun8;
    FndRun8 = Fiend 3, 3, Chase, FndRun1;
    FndAtk1 = Fiend 4, 8, FaceTarget, FndAtk2;
    FndAtk2 = Fiend 4, 8, FaceTarget, FndAtk3;
    FndAtk3 = Fiend 5 | B, 6, FiendAttack, FndRun1;
    FndPain = Fiend 6, 2, None, FndPain2;
    FndPain2 = Fiend 6, 2, Pain, FndRun1;
    FndDie1 = Fiend 7, 8, None, FndDie2;
    FndDie2 = Fiend 8, 8, Scream, FndDie3;
    FndDie3 = Fiend 9, 6, None, FndDie4;
    FndDie4 = Fiend 10, 6, Fall, FndDie5;
    FndDie5 = Fiend 11, -1, None, Null;
    FndXDie1 = Fiend 12, 5, None, FndXDie2;
    FndXDie2 = Fiend 13, 5, XScream, FndXDie3;
    FndXDie3 = Fiend 14, 5, Fall, FndXDie4;
    FndXDie4 = Fiend 15, 5, None, FndXDie5;
    FndXDie5 = Fiend 16, -1, None, Null;
    // ---- ripper (charging biter)
    RipStnd = Ripper 0, 10, Look, RipStnd2;
    RipStnd2 = Ripper 1, 10, Look, RipStnd;
    RipRun1 = Ripper 0, 2, Chase, RipRun2;
    RipRun2 = Ripper 0, 2, Chase, RipRun3;
    RipRun3 = Ripper 1, 2, Chase, RipRun4;
    RipRun4 = Ripper 1, 2, Chase, RipRun5;
    RipRun5 = Ripper 2, 2, Chase, RipRun6;
    RipRun6 = Ripper 2, 2, Chase, RipRun7;
    RipRun7 = Ripper 3, 2, Chase, RipRun8;
    RipRun8 = Ripper 3, 2, Chase, RipRun1;
    RipAtk1 = Ripper 4, 8, FaceTarget, RipAtk2;
    RipAtk2 = Ripper 5, 8, FaceTarget, RipAtk3;
    RipAtk3 = Ripper 5, 8, RipperBite, RipRun1;
    RipPain = Ripper 6, 2, None, RipPain2;
    RipPain2 = Ripper 6, 2, Pain, RipRun1;
    RipDie1 = Ripper 7, 8, None, RipDie2;
    RipDie2 = Ripper 8, 8, Scream, RipDie3;
    RipDie3 = Ripper 9, 4, None, RipDie4;
    RipDie4 = Ripper 10, 4, Fall, RipDie5;
    RipDie5 = Ripper 11, -1, None, Null;
    // ---- gazer (floating bell, plasma spit)
    GazStnd = Gazer 0, 10, Look, GazStnd2;
    GazStnd2 = Gazer 1, 10, Look, GazStnd;
    GazRun1 = Gazer 0, 3, Chase, GazRun2;
    GazRun2 = Gazer 1, 3, Chase, GazRun3;
    GazRun3 = Gazer 2, 3, Chase, GazRun4;
    GazRun4 = Gazer 3, 3, Chase, GazRun1;
    GazAtk1 = Gazer 4, 5, FaceTarget, GazAtk2;
    GazAtk2 = Gazer 4, 5, FaceTarget, GazAtk3;
    GazAtk3 = Gazer 5 | B, 5, GazerAttack, GazRun1;
    GazPain = Gazer 6, 3, None, GazPain2;
    GazPain2 = Gazer 6, 3, Pain, GazPain3;
    GazPain3 = Gazer 6, 6, None, GazRun1;
    GazDie1 = Gazer 7, 8, None, GazDie2;
    GazDie2 = Gazer 8, 8, Scream, GazDie3;
    GazDie3 = Gazer 9, 8, None, GazDie4;
    GazDie4 = Gazer 10, 8, Fall, GazDie5;
    GazDie5 = Gazer 11, -1, None, Null;
    // ---- juggernaut (armoured brute, energy orbs)
    JugStnd = Juggernaut 0, 10, Look, JugStnd2;
    JugStnd2 = Juggernaut 1, 10, Look, JugStnd;
    JugRun1 = Juggernaut 0, 3, Chase, JugRun2;
    JugRun2 = Juggernaut 0, 3, Chase, JugRun3;
    JugRun3 = Juggernaut 1, 3, Chase, JugRun4;
    JugRun4 = Juggernaut 1, 3, Chase, JugRun5;
    JugRun5 = Juggernaut 2, 3, Chase, JugRun6;
    JugRun6 = Juggernaut 2, 3, Chase, JugRun7;
    JugRun7 = Juggernaut 3, 3, Chase, JugRun8;
    JugRun8 = Juggernaut 3, 3, Chase, JugRun1;
    JugAtk1 = Juggernaut 4, 8, FaceTarget, JugAtk2;
    JugAtk2 = Juggernaut 4, 8, FaceTarget, JugAtk3;
    JugAtk3 = Juggernaut 5 | B, 8, JuggAttack, JugRun1;
    JugPain = Juggernaut 6, 2, None, JugPain2;
    JugPain2 = Juggernaut 6, 2, Pain, JugRun1;
    JugDie1 = Juggernaut 7, 8, None, JugDie2;
    JugDie2 = Juggernaut 8, 8, Scream, JugDie3;
    JugDie3 = Juggernaut 9, 8, None, JugDie4;
    JugDie4 = Juggernaut 10, 8, Fall, JugDie5;
    JugDie5 = Juggernaut 11, 8, None, JugDie6;
    JugDie6 = Juggernaut 11, -1, BossDeath, Null;
    // ---- projectiles
    FBall1 = FiendBall 0 | B, 4, None, FBall2;
    FBall2 = FiendBall 1 | B, 4, None, FBall1;
    FBallX1 = FiendBall 2 | B, 6, None, FBallX2;
    FBallX2 = FiendBall 3 | B, 6, None, FBallX3;
    FBallX3 = FiendBall 4 | B, 6, None, Null;
    GBall1 = GazerBall 0 | B, 4, None, GBall2;
    GBall2 = GazerBall 1 | B, 4, None, GBall1;
    GBallX1 = GazerBall 2 | B, 6, None, GBallX2;
    GBallX2 = GazerBall 3 | B, 6, None, GBallX3;
    GBallX3 = GazerBall 4 | B, 6, None, Null;
    JBall1 = JuggBall 0 | B, 4, None, JBall2;
    JBall2 = JuggBall 1 | B, 4, None, JBall1;
    JBallX1 = JuggBall 2 | B, 6, None, JBallX2;
    JBallX2 = JuggBall 3 | B, 6, None, JBallX3;
    JBallX3 = JuggBall 4 | B, 6, None, Null;
    Rocket1 = Rocket 0 | B, 1, None, Rocket1;
    RocketX1 = Rocket 1 | B, 8, Explode, RocketX2;
    RocketX2 = Rocket 2 | B, 6, None, RocketX3;
    RocketX3 = Rocket 3 | B, 4, None, Null;
    Plasma1 = Plasma 0 | B, 6, None, Plasma2;
    Plasma2 = Plasma 1 | B, 6, None, Plasma1;
    PlasmaX1 = Plasma 2 | B, 4, None, PlasmaX2;
    PlasmaX2 = Plasma 3 | B, 4, None, PlasmaX3;
    PlasmaX3 = Plasma 4 | B, 4, None, PlasmaX4;
    PlasmaX4 = Plasma 5 | B, 4, None, Null;
    Arc1 = ArcBall 0 | B, 4, None, Arc2;
    Arc2 = ArcBall 1 | B, 4, None, Arc1;
    ArcX1 = ArcBall 2 | B, 8, None, ArcX2;
    ArcX2 = ArcBall 3 | B, 8, None, ArcX3;
    ArcX3 = ArcBall 4 | B, 8, ArcSpray, ArcX4;
    ArcX4 = ArcBall 5 | B, 8, None, ArcX5;
    ArcX5 = ArcBall 6 | B, 8, None, Null;
    ArcT1 = ArcTrace 0 | B, 8, None, ArcT2;
    ArcT2 = ArcTrace 1 | B, 8, None, ArcT3;
    ArcT3 = ArcTrace 2 | B, 8, None, Null;
    // ---- effects
    Puff1 = Puff 0 | B, 4, None, Puff2;
    Puff2 = Puff 1, 4, None, Puff3;
    Puff3 = Puff 2, 4, None, Puff4;
    Puff4 = Puff 3, 4, None, Null;
    Blood1 = Blood 2, 8, None, Blood2;
    Blood2 = Blood 1, 8, None, Blood3;
    Blood3 = Blood 0, 8, None, Null;
    Fog1 = Fog 0 | B, 6, None, Fog2;
    Fog2 = Fog 1 | B, 6, None, Fog3;
    Fog3 = Fog 2 | B, 6, None, Fog4;
    Fog4 = Fog 3 | B, 6, None, Fog5;
    Fog5 = Fog 4 | B, 6, None, Fog6;
    Fog6 = Fog 5 | B, 6, None, Null;
    // ---- decorations
    Barrel1 = Barrel 0, 6, None, Barrel2;
    Barrel2 = Barrel 1, 6, None, Barrel1;
    BarrelX1 = Barrel 2 | B, 5, None, BarrelX2;
    BarrelX2 = Barrel 3 | B, 5, Scream, BarrelX3;
    BarrelX3 = Barrel 4 | B, 5, None, BarrelX4;
    BarrelX4 = Barrel 5 | B, 10, Explode, BarrelX5;
    BarrelX5 = Barrel 6 | B, 10, None, Null;
    Lamp1 = Lamp 0 | B, -1, None, Lamp1;
    Pillar1 = Pillar 0, -1, None, Pillar1;
    Torch1 = Torch 0 | B, 4, None, Torch2;
    Torch2 = Torch 1 | B, 4, None, Torch3;
    Torch3 = Torch 2 | B, 4, None, Torch4;
    Torch4 = Torch 3 | B, 4, None, Torch1;
    Corpse1 = Corpse 0, -1, None, Corpse1;
    // ---- items
    Clip1 = Clip 0, -1, None, Clip1;
    AmmoBox1 = AmmoBox 0, -1, None, AmmoBox1;
    Shells1 = Shells 0, -1, None, Shells1;
    ShellBox1 = ShellBox 0, -1, None, ShellBox1;
    RocketAmmo1 = RocketAmmo 0, -1, None, RocketAmmo1;
    RocketBox1 = RocketBox 0, -1, None, RocketBox1;
    Cell1 = Cell 0 | B, -1, None, Cell1;
    CellPack1 = CellPack 0 | B, -1, None, CellPack1;
    Backpack1 = Backpack 0, -1, None, Backpack1;
    Stim1 = Stim 0, -1, None, Stim1;
    Medkit1 = Medkit 0, -1, None, Medkit1;
    HBonus1 = HealthBonus 0 | B, 6, None, HBonus2;
    HBonus2 = HealthBonus 1 | B, 6, None, HBonus3;
    HBonus3 = HealthBonus 2 | B, 6, None, HBonus4;
    HBonus4 = HealthBonus 1 | B, 6, None, HBonus1;
    ABonus1 = ArmorBonus 0 | B, 6, None, ABonus2;
    ABonus2 = ArmorBonus 1 | B, 6, None, ABonus3;
    ABonus3 = ArmorBonus 2 | B, 6, None, ABonus4;
    ABonus4 = ArmorBonus 1 | B, 6, None, ABonus1;
    ArmorG1 = ArmorGreen 0, 6, None, ArmorG2;
    ArmorG2 = ArmorGreen 1 | B, 7, None, ArmorG1;
    ArmorB1 = ArmorBlue 0, 6, None, ArmorB2;
    ArmorB2 = ArmorBlue 1 | B, 6, None, ArmorB1;
    Orb1 = VitalOrb 0 | B, 6, None, Orb2;
    Orb2 = VitalOrb 1 | B, 6, None, Orb3;
    Orb3 = VitalOrb 2 | B, 6, None, Orb4;
    Orb4 = VitalOrb 3 | B, 6, None, Orb1;
    KeyR1 = KeyRed 0, 10, None, KeyR2;
    KeyR2 = KeyRed 1 | B, 10, None, KeyR1;
    KeyB1 = KeyBlue 0, 10, None, KeyB2;
    KeyB2 = KeyBlue 1 | B, 10, None, KeyB1;
    KeyY1 = KeyYellow 0, 10, None, KeyY2;
    KeyY2 = KeyYellow 1 | B, 10, None, KeyY1;
    PShotgun1 = PShotgun 0, -1, None, PShotgun1;
    PChaingun1 = PChaingun 0, -1, None, PChaingun1;
    PLauncher1 = PLauncher 0, -1, None, PLauncher1;
    PPlasma1 = PPlasma 0, -1, None, PPlasma1;
    PArc1 = PArc 0, -1, None, PArc1;
    PDrill1 = PDrill 0, -1, None, PDrill1;
    Aegis1 = Aegis 0 | B, 6, None, Aegis2;
    Aegis2 = Aegis 1 | B, 6, None, Aegis1;
    Hazmat1 = Hazmat 0 | B, -1, None, Hazmat1;
    Survey1 = SurveyMap 0 | B, 6, None, Survey2;
    Survey2 = SurveyMap 1 | B, 6, None, Survey1;
    Adren1 = Adrenaline 0 | B, -1, None, Adren1;
    // ---- first-person weapons (psprites)
    Light0 = None 0, 0, Light0, Null;
    FistReady = WFist 0, 1, WeaponReady, FistReady;
    FistDown = WFist 0, 1, Lower, FistDown;
    FistUp = WFist 0, 1, Raise, FistUp;
    FistAtk1 = WFist 1, 4, None, FistAtk2;
    FistAtk2 = WFist 2, 4, FirePunch, FistAtk3;
    FistAtk3 = WFist 3, 5, None, FistAtk4;
    FistAtk4 = WFist 2, 4, None, FistAtk5;
    FistAtk5 = WFist 1, 5, ReFire, FistReady;
    DrillReady = WDrill 0, 4, WeaponReady, DrillReady2;
    DrillReady2 = WDrill 1, 4, WeaponReady, DrillReady;
    DrillDown = WDrill 0, 1, Lower, DrillDown;
    DrillUp = WDrill 0, 1, Raise, DrillUp;
    DrillAtk1 = WDrill 2, 4, FireDrill, DrillAtk2;
    DrillAtk2 = WDrill 3, 4, FireDrill, DrillAtk3;
    DrillAtk3 = WDrill 3, 0, ReFire, DrillReady;
    PistolReady = WPistol 0, 1, WeaponReady, PistolReady;
    PistolDown = WPistol 0, 1, Lower, PistolDown;
    PistolUp = WPistol 0, 1, Raise, PistolUp;
    PistolAtk1 = WPistol 0, 4, None, PistolAtk2;
    PistolAtk2 = WPistol 1, 6, FirePistol, PistolAtk3;
    PistolAtk3 = WPistol 2, 4, None, PistolAtk4;
    PistolAtk4 = WPistol 1, 5, ReFire, PistolReady;
    PistolFlash = WPistol 3 | B, 7, Light1, Light0;
    SgReady = WShotgun 0, 1, WeaponReady, SgReady;
    SgDown = WShotgun 0, 1, Lower, SgDown;
    SgUp = WShotgun 0, 1, Raise, SgUp;
    SgAtk1 = WShotgun 0, 3, None, SgAtk2;
    SgAtk2 = WShotgun 0, 7, FireShotgun, SgAtk3;
    SgAtk3 = WShotgun 1, 5, None, SgAtk4;
    SgAtk4 = WShotgun 2, 4, None, SgAtk5;
    SgAtk5 = WShotgun 3, 4, None, SgAtk6;
    SgAtk6 = WShotgun 2, 4, None, SgAtk7;
    SgAtk7 = WShotgun 1, 4, None, SgAtk8;
    SgAtk8 = WShotgun 0, 3, None, SgAtk9;
    SgAtk9 = WShotgun 0, 3, ReFire, SgReady;
    SgFlash1 = WShotgun 4 | B, 4, Light1, SgFlash2;
    SgFlash2 = WShotgun 5 | B, 3, Light2, Light0;
    CgReady = WChaingun 0, 1, WeaponReady, CgReady;
    CgDown = WChaingun 0, 1, Lower, CgDown;
    CgUp = WChaingun 0, 1, Raise, CgUp;
    CgAtk1 = WChaingun 0, 4, FireChaingun, CgAtk2;
    CgAtk2 = WChaingun 1, 4, FireChaingun, CgAtk3;
    CgAtk3 = WChaingun 1, 0, ReFire, CgReady;
    CgFlash1 = WChaingun 2 | B, 5, Light1, Light0;
    CgFlash2 = WChaingun 3 | B, 5, Light2, Light0;
    RlReady = WLauncher 0, 1, WeaponReady, RlReady;
    RlDown = WLauncher 0, 1, Lower, RlDown;
    RlUp = WLauncher 0, 1, Raise, RlUp;
    RlAtk1 = WLauncher 1, 8, GunFlash, RlAtk2;
    RlAtk2 = WLauncher 1, 12, FireLauncher, RlAtk3;
    RlAtk3 = WLauncher 0, 0, ReFire, RlReady;
    RlFlash1 = WLauncher 2 | B, 3, Light1, RlFlash2;
    RlFlash2 = WLauncher 3 | B, 4, None, RlFlash3;
    RlFlash3 = WLauncher 4 | B, 4, Light2, RlFlash4;
    RlFlash4 = WLauncher 5 | B, 4, Light2, Light0;
    PlReady = WPlasma 0, 1, WeaponReady, PlReady;
    PlDown = WPlasma 0, 1, Lower, PlDown;
    PlUp = WPlasma 0, 1, Raise, PlUp;
    PlAtk1 = WPlasma 0, 3, FirePlasma, PlAtk2;
    PlAtk2 = WPlasma 1, 20, ReFire, PlReady;
    PlFlash1 = WPlasma 2 | B, 4, Light1, Light0;
    PlFlash2 = WPlasma 3 | B, 4, Light1, Light0;
    ArcReady = WArc 0, 1, WeaponReady, ArcReady;
    ArcDown = WArc 0, 1, Lower, ArcDown;
    ArcUp = WArc 0, 1, Raise, ArcUp;
    ArcAtk1 = WArc 0, 20, ArcCharge, ArcAtk2;
    ArcAtk2 = WArc 1, 10, GunFlash, ArcAtk3;
    ArcAtk3 = WArc 1, 10, FireArc, ArcAtk4;
    ArcAtk4 = WArc 1, 20, ReFire, ArcReady;
    ArcFlash1 = WArc 2 | B, 11, Light1, ArcFlash2;
    ArcFlash2 = WArc 3 | B, 6, Light2, Light0;
}

// ------------------------------------------------------------------ thing kinds

/// Every kind of thing that can exist. Names are what level files use
/// (`thing fiend 128 256 90` -> `ThingKind::Fiend`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ThingKind {
    Player1,
    Drone,
    Enforcer,
    Heavy,
    Fiend,
    Ripper,
    Gazer,
    Juggernaut,
    Barrel,
    FiendBall,
    GazerBall,
    JuggBall,
    Rocket,
    PlasmaShot,
    ArcShot,
    ArcTrace,
    Puff,
    Blood,
    TeleFog,
    TeleportDest,
    Lamp,
    Pillar,
    Torch,
    Corpse,
    Clip,
    AmmoBox,
    Shells,
    ShellBox,
    Rockets,
    RocketBox,
    Cell,
    CellPack,
    Backpack,
    Stimpack,
    Medkit,
    HealthBonus,
    ArmorBonus,
    ArmorGreen,
    ArmorBlue,
    VitalOrb,
    KeyRed,
    KeyBlue,
    KeyYellow,
    Shotgun,
    Chaingun,
    Launcher,
    Plasma,
    ArcCannon,
    Drill,
    Aegis,
    Hazmat,
    SurveyMap,
    Adrenaline,
}

pub const NUM_KINDS: usize = ThingKind::Adrenaline as usize + 1;

impl ThingKind {
    pub fn from_u8(v: u8) -> ThingKind {
        if (v as usize) < NUM_KINDS {
            // SAFETY: repr(u8) enum with contiguous discriminants 0..NUM_KINDS.
            unsafe { core::mem::transmute::<u8, ThingKind>(v) }
        } else {
            ThingKind::Puff
        }
    }
}

pub struct MobjInfo {
    pub spawn: S,
    pub health: i32,
    pub see: S,
    pub reaction: i16,
    pub pain: S,
    pub painchance: u8,
    pub melee: S,
    pub missile: S,
    pub death: S,
    pub xdeath: S,
    /// Units per tic for monsters (map units), fixed-point speed for missiles.
    pub speed: Fixed,
    pub radius: Fixed,
    pub height: Fixed,
    pub mass: i32,
    pub damage: i32,
    pub flags: u32,
}

const F: Fixed = FRACUNIT;

const fn item(spawn: S, flags: u32) -> MobjInfo {
    MobjInfo {
        spawn,
        health: 1000,
        see: S::Null,
        reaction: 8,
        pain: S::Null,
        painchance: 0,
        melee: S::Null,
        missile: S::Null,
        death: S::Null,
        xdeath: S::Null,
        speed: 0,
        radius: 20 * F,
        height: 16 * F,
        mass: 100,
        damage: 0,
        flags: MF_SPECIAL | flags,
    }
}

const fn deco(spawn: S, radius: i32, height: i32, flags: u32) -> MobjInfo {
    MobjInfo {
        spawn,
        health: 1000,
        see: S::Null,
        reaction: 8,
        pain: S::Null,
        painchance: 0,
        melee: S::Null,
        missile: S::Null,
        death: S::Null,
        xdeath: S::Null,
        speed: 0,
        radius: radius * F,
        height: height * F,
        mass: 100,
        damage: 0,
        flags,
    }
}

const fn missile(spawn: S, death: S, speed: i32, radius: i32, damage: i32) -> MobjInfo {
    MobjInfo {
        spawn,
        health: 1000,
        see: S::Null,
        reaction: 8,
        pain: S::Null,
        painchance: 0,
        melee: S::Null,
        missile: S::Null,
        death,
        xdeath: S::Null,
        speed: speed * F,
        radius: radius * F,
        height: 8 * F,
        mass: 100,
        damage,
        flags: MF_NOBLOCKMAP | MF_MISSILE | MF_DROPOFF | MF_NOGRAVITY,
    }
}

const fn fx_only(spawn: S) -> MobjInfo {
    deco(spawn, 20, 16, MF_NOBLOCKMAP | MF_NOGRAVITY)
}

#[allow(clippy::too_many_arguments)]
const fn monster(
    spawn: S,
    health: i32,
    see: S,
    pain: S,
    painchance: u8,
    melee: S,
    missile: S,
    death: S,
    xdeath: S,
    speed: i32,
    radius: i32,
    height: i32,
    mass: i32,
    flags: u32,
) -> MobjInfo {
    MobjInfo {
        spawn,
        health,
        see,
        reaction: 8,
        pain,
        painchance,
        melee,
        missile,
        death,
        xdeath,
        speed: speed * F,
        radius: radius * F,
        height: height * F,
        mass,
        damage: 0,
        flags: MF_SOLID | MF_SHOOTABLE | MF_COUNTKILL | flags,
    }
}

pub static MOBJINFO: [MobjInfo; NUM_KINDS] = [
    // Player1
    MobjInfo {
        spawn: S::PlayStand,
        health: 100,
        see: S::PlayRun1,
        reaction: 0,
        pain: S::PlayPain,
        painchance: 255,
        melee: S::Null,
        missile: S::PlayAtk1,
        death: S::PlayDie1,
        xdeath: S::PlayXDie1,
        speed: 0,
        radius: 16 * F,
        height: 56 * F,
        mass: 100,
        damage: 0,
        flags: MF_SOLID | MF_SHOOTABLE | MF_DROPOFF | MF_PICKUP | MF_NOTDMATCH,
    },
    monster(S::DroneStnd, 20, S::DroneRun1, S::DronePain, 200, S::Null, S::DroneAtk1, S::DroneDie1, S::DroneXDie1, 8, 20, 56, 100, 0),
    monster(S::EnfStnd, 30, S::EnfRun1, S::EnfPain, 170, S::Null, S::EnfAtk1, S::EnfDie1, S::EnfXDie1, 8, 20, 56, 100, 0),
    monster(S::HvyStnd, 70, S::HvyRun1, S::HvyPain, 170, S::Null, S::HvyAtk1, S::HvyDie1, S::HvyXDie1, 8, 20, 56, 100, 0),
    monster(S::FndStnd, 60, S::FndRun1, S::FndPain, 200, S::FndAtk1, S::FndAtk1, S::FndDie1, S::FndXDie1, 8, 20, 56, 100, 0),
    monster(S::RipStnd, 150, S::RipRun1, S::RipPain, 180, S::RipAtk1, S::Null, S::RipDie1, S::Null, 10, 30, 56, 400, 0),
    monster(S::GazStnd, 400, S::GazRun1, S::GazPain, 128, S::Null, S::GazAtk1, S::GazDie1, S::Null, 8, 31, 56, 400, MF_FLOAT | MF_NOGRAVITY),
    monster(S::JugStnd, 1000, S::JugRun1, S::JugPain, 50, S::JugAtk1, S::JugAtk1, S::JugDie1, S::Null, 8, 24, 64, 1000, MF_BOSS),
    // Barrel
    MobjInfo {
        spawn: S::Barrel1,
        health: 20,
        see: S::Null,
        reaction: 8,
        pain: S::Null,
        painchance: 0,
        melee: S::Null,
        missile: S::Null,
        death: S::BarrelX1,
        xdeath: S::Null,
        speed: 0,
        radius: 10 * F,
        height: 42 * F,
        mass: 100,
        damage: 0,
        flags: MF_SOLID | MF_SHOOTABLE | MF_NOBLOOD,
    },
    missile(S::FBall1, S::FBallX1, 10, 6, 3),
    missile(S::GBall1, S::GBallX1, 10, 6, 5),
    missile(S::JBall1, S::JBallX1, 15, 6, 8),
    missile(S::Rocket1, S::RocketX1, 20, 11, 20),
    missile(S::Plasma1, S::PlasmaX1, 25, 13, 5),
    missile(S::Arc1, S::ArcX1, 25, 13, 100),
    fx_only(S::ArcT1),
    fx_only(S::Puff1),
    deco(S::Blood1, 20, 16, MF_NOBLOCKMAP),
    fx_only(S::Fog1),
    deco(S::Null, 20, 16, MF_NOSECTOR | MF_NOBLOCKMAP),
    deco(S::Lamp1, 16, 48, MF_SOLID),
    deco(S::Pillar1, 16, 52, MF_SOLID),
    deco(S::Torch1, 16, 64, MF_SOLID),
    deco(S::Corpse1, 20, 16, 0),
    item(S::Clip1, 0),
    item(S::AmmoBox1, 0),
    item(S::Shells1, 0),
    item(S::ShellBox1, 0),
    item(S::RocketAmmo1, 0),
    item(S::RocketBox1, 0),
    item(S::Cell1, 0),
    item(S::CellPack1, 0),
    item(S::Backpack1, 0),
    item(S::Stim1, 0),
    item(S::Medkit1, 0),
    item(S::HBonus1, MF_COUNTITEM),
    item(S::ABonus1, MF_COUNTITEM),
    item(S::ArmorG1, 0),
    item(S::ArmorB1, 0),
    item(S::Orb1, MF_COUNTITEM),
    item(S::KeyR1, MF_NOTDMATCH),
    item(S::KeyB1, MF_NOTDMATCH),
    item(S::KeyY1, MF_NOTDMATCH),
    item(S::PShotgun1, 0),
    item(S::PChaingun1, 0),
    item(S::PLauncher1, 0),
    item(S::PPlasma1, 0),
    item(S::PArc1, 0),
    item(S::PDrill1, 0),
    item(S::Aegis1, MF_COUNTITEM),
    item(S::Hazmat1, 0),
    item(S::Survey1, MF_COUNTITEM),
    item(S::Adren1, MF_COUNTITEM),
];

pub fn info(k: ThingKind) -> &'static MobjInfo {
    &MOBJINFO[k as usize]
}

pub fn state(s: S) -> &'static State {
    &STATES[s as usize]
}
