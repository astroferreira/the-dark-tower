//! Body templates for different creature types
//!
//! Provides pre-built body configurations for humanoids, quadrupeds, dragons, etc.

use super::parts::{
    BodyPart, BodyPartCategory, BodyPartFunction, BodyPartId, BodyPartSize, Tissue,
};
use std::collections::HashMap;

/// Type of body plan
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyPlan {
    Humanoid,
    Quadruped,
    Dragon,
    Arachnid,
    Insectoid,
    Avian,
    Serpentine,
    Worm,
    Spectral,
}

impl BodyPlan {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Humanoid => "humanoid",
            Self::Quadruped => "quadruped",
            Self::Dragon => "dragon",
            Self::Arachnid => "arachnid",
            Self::Insectoid => "insectoid",
            Self::Avian => "avian",
            Self::Serpentine => "serpentine",
            Self::Worm => "worm",
            Self::Spectral => "spectral",
        }
    }
}

/// Complete body structure
#[derive(Debug, Clone)]
pub struct Body {
    pub plan: BodyPlan,
    pub parts: HashMap<BodyPartId, BodyPart>,
    pub blood: f32,
    pub max_blood: f32,
    pub blood_loss_rate: f32,
    next_part_id: u32,
}

impl Body {
    fn new(plan: BodyPlan, max_blood: f32) -> Self {
        Self {
            plan,
            parts: HashMap::new(),
            blood: max_blood,
            max_blood,
            blood_loss_rate: 0.0,
            next_part_id: 0,
        }
    }

    fn next_id(&mut self) -> BodyPartId {
        let id = BodyPartId::new(self.next_part_id);
        self.next_part_id += 1;
        id
    }

    fn add_part(&mut self, part: BodyPart) -> BodyPartId {
        let id = part.id;
        self.parts.insert(id, part);
        id
    }

    fn link_child(&mut self, parent: BodyPartId, child: BodyPartId) {
        if let Some(parent_part) = self.parts.get_mut(&parent) {
            parent_part.children.push(child);
        }
        if let Some(child_part) = self.parts.get_mut(&child) {
            child_part.parent = Some(parent);
        }
    }

    /// Create a humanoid body (humans, trolls, yeti)
    pub fn humanoid(tissue: Tissue) -> Self {
        let mut body = Body::new(BodyPlan::Humanoid, 100.0);

        // Torso
        let torso_id = body.next_id();
        let torso = BodyPart::new(
            torso_id,
            "torso",
            BodyPartCategory::Torso,
            BodyPartSize::Large,
            tissue,
            true,
        )
        .with_functions(&[BodyPartFunction::Breathing]);
        body.add_part(torso);

        // Head
        let head_id = body.next_id();
        let head = BodyPart::new(
            head_id,
            "head",
            BodyPartCategory::Head,
            BodyPartSize::Medium,
            tissue,
            true,
        )
        .with_functions(&[BodyPartFunction::Thinking, BodyPartFunction::Breathing])
        .with_parent(torso_id);
        body.add_part(head);
        body.link_child(torso_id, head_id);

        // Eyes
        for side in &["left", "right"] {
            let eye_id = body.next_id();
            let eye = BodyPart::new(
                eye_id,
                format!("{} eye", side),
                BodyPartCategory::Sensory,
                BodyPartSize::Tiny,
                Tissue::Flesh,
                false,
            )
            .with_functions(&[BodyPartFunction::Vision])
            .with_parent(head_id);
            body.add_part(eye);
            body.link_child(head_id, eye_id);
        }

        // Arms
        for side in &["left", "right"] {
            let arm_id = body.next_id();
            let arm = BodyPart::new(
                arm_id,
                format!("{} arm", side),
                BodyPartCategory::UpperLimb,
                BodyPartSize::Medium,
                tissue,
                false,
            )
            .with_parent(torso_id);
            body.add_part(arm);
            body.link_child(torso_id, arm_id);

            // Hand
            let hand_id = body.next_id();
            let hand = BodyPart::new(
                hand_id,
                format!("{} hand", side),
                BodyPartCategory::Extremity,
                BodyPartSize::Small,
                tissue,
                false,
            )
            .with_functions(&[BodyPartFunction::Grasping])
            .with_parent(arm_id);
            body.add_part(hand);
            body.link_child(arm_id, hand_id);
        }

        // Legs
        for side in &["left", "right"] {
            let leg_id = body.next_id();
            let leg = BodyPart::new(
                leg_id,
                format!("{} leg", side),
                BodyPartCategory::LowerLimb,
                BodyPartSize::Medium,
                tissue,
                false,
            )
            .with_functions(&[BodyPartFunction::Locomotion])
            .with_parent(torso_id);
            body.add_part(leg);
            body.link_child(torso_id, leg_id);

            // Foot
            let foot_id = body.next_id();
            let foot = BodyPart::new(
                foot_id,
                format!("{} foot", side),
                BodyPartCategory::Extremity,
                BodyPartSize::Small,
                tissue,
                false,
            )
            .with_functions(&[BodyPartFunction::Locomotion, BodyPartFunction::Balance])
            .with_parent(leg_id);
            body.add_part(foot);
            body.link_child(leg_id, foot_id);
        }

        body
    }

    /// Create a quadruped body (wolves, bears)
    pub fn quadruped(tissue: Tissue) -> Self {
        let mut body = Body::new(BodyPlan::Quadruped, 80.0);

        // Torso
        let torso_id = body.next_id();
        let torso = BodyPart::new(
            torso_id,
            "torso",
            BodyPartCategory::Torso,
            BodyPartSize::Large,
            tissue,
            true,
        )
        .with_functions(&[BodyPartFunction::Breathing]);
        body.add_part(torso);

        // Head
        let head_id = body.next_id();
        let head = BodyPart::new(
            head_id,
            "head",
            BodyPartCategory::Head,
            BodyPartSize::Medium,
            tissue,
            true,
        )
        .with_functions(&[
            BodyPartFunction::Thinking,
            BodyPartFunction::Breathing,
            BodyPartFunction::Attacking,
        ])
        .with_parent(torso_id);
        body.add_part(head);
        body.link_child(torso_id, head_id);

        // Eyes
        for side in &["left", "right"] {
            let eye_id = body.next_id();
            let eye = BodyPart::new(
                eye_id,
                format!("{} eye", side),
                BodyPartCategory::Sensory,
                BodyPartSize::Tiny,
                Tissue::Flesh,
                false,
            )
            .with_functions(&[BodyPartFunction::Vision])
            .with_parent(head_id);
            body.add_part(eye);
            body.link_child(head_id, eye_id);
        }

        // Four legs
        for (position, name) in &[
            ("front", "left foreleg"),
            ("front", "right foreleg"),
            ("back", "left hind leg"),
            ("back", "right hind leg"),
        ] {
            let leg_id = body.next_id();
            let leg = BodyPart::new(
                leg_id,
                *name,
                BodyPartCategory::LowerLimb,
                BodyPartSize::Medium,
                tissue,
                false,
            )
            .with_functions(&[BodyPartFunction::Locomotion])
            .with_parent(torso_id);
            body.add_part(leg);
            body.link_child(torso_id, leg_id);

            // Paw
            let paw_id = body.next_id();
            let paw_name = if *position == "front" {
                name.replace("foreleg", "forepaw")
            } else {
                name.replace("hind leg", "hind paw")
            };
            let paw = BodyPart::new(
                paw_id,
                paw_name,
                BodyPartCategory::Extremity,
                BodyPartSize::Small,
                tissue,
                false,
            )
            .with_functions(&[BodyPartFunction::Locomotion, BodyPartFunction::Attacking])
            .with_parent(leg_id);
            body.add_part(paw);
            body.link_child(leg_id, paw_id);
        }

        // Tail
        let tail_id = body.next_id();
        let tail = BodyPart::new(
            tail_id,
            "tail",
            BodyPartCategory::Tail,
            BodyPartSize::Small,
            tissue,
            false,
        )
        .with_functions(&[BodyPartFunction::Balance])
        .with_parent(torso_id);
        body.add_part(tail);
        body.link_child(torso_id, tail_id);

        body
    }

    /// Create a dragon body
    pub fn dragon() -> Self {
        let mut body = Body::new(BodyPlan::Dragon, 300.0);
        let tissue = Tissue::Scale;

        // Torso
        let torso_id = body.next_id();
        let torso = BodyPart::new(
            torso_id,
            "torso",
            BodyPartCategory::Torso,
            BodyPartSize::Huge,
            tissue,
            true,
        )
        .with_functions(&[BodyPartFunction::Breathing]);
        body.add_part(torso);

        // Head
        let head_id = body.next_id();
        let head = BodyPart::new(
            head_id,
            "head",
            BodyPartCategory::Head,
            BodyPartSize::Large,
            tissue,
            true,
        )
        .with_functions(&[
            BodyPartFunction::Thinking,
            BodyPartFunction::Breathing,
            BodyPartFunction::Attacking,
            BodyPartFunction::FireBreath,
        ])
        .with_parent(torso_id);
        body.add_part(head);
        body.link_child(torso_id, head_id);

        // Eyes
        for side in &["left", "right"] {
            let eye_id = body.next_id();
            let eye = BodyPart::new(
                eye_id,
                format!("{} eye", side),
                BodyPartCategory::Sensory,
                BodyPartSize::Small,
                Tissue::Flesh,
                false,
            )
            .with_functions(&[BodyPartFunction::Vision])
            .with_parent(head_id);
            body.add_part(eye);
            body.link_child(head_id, eye_id);
        }

        // Wings
        for side in &["left", "right"] {
            let wing_id = body.next_id();
            let wing = BodyPart::new(
                wing_id,
                format!("{} wing", side),
                BodyPartCategory::Wing,
                BodyPartSize::Large,
                tissue,
                false,
            )
            .with_functions(&[BodyPartFunction::Flight])
            .with_parent(torso_id);
            body.add_part(wing);
            body.link_child(torso_id, wing_id);
        }

        // Four legs
        for name in &[
            "left foreleg",
            "right foreleg",
            "left hind leg",
            "right hind leg",
        ] {
            let leg_id = body.next_id();
            let leg = BodyPart::new(
                leg_id,
                *name,
                BodyPartCategory::LowerLimb,
                BodyPartSize::Large,
                tissue,
                false,
            )
            .with_functions(&[BodyPartFunction::Locomotion, BodyPartFunction::Attacking])
            .with_parent(torso_id);
            body.add_part(leg);
            body.link_child(torso_id, leg_id);
        }

        // Tail
        let tail_id = body.next_id();
        let tail = BodyPart::new(
            tail_id,
            "tail",
            BodyPartCategory::Tail,
            BodyPartSize::Large,
            tissue,
            false,
        )
        .with_functions(&[BodyPartFunction::Balance, BodyPartFunction::Attacking])
        .with_parent(torso_id);
        body.add_part(tail);
        body.link_child(torso_id, tail_id);

        body
    }

    /// Create an arachnid body (giant spiders)
    pub fn arachnid(tissue: Tissue) -> Self {
        let mut body = Body::new(BodyPlan::Arachnid, 40.0);

        // Cephalothorax (combined head/torso)
        let ceph_id = body.next_id();
        let ceph = BodyPart::new(
            ceph_id,
            "cephalothorax",
            BodyPartCategory::Torso,
            BodyPartSize::Medium,
            tissue,
            true,
        )
        .with_functions(&[
            BodyPartFunction::Breathing,
            BodyPartFunction::Thinking,
            BodyPartFunction::Attacking,
        ]);
        body.add_part(ceph);

        // Eyes (8 for spider)
        for i in 1..=8 {
            let eye_id = body.next_id();
            let eye = BodyPart::new(
                eye_id,
                format!("eye {}", i),
                BodyPartCategory::Sensory,
                BodyPartSize::Tiny,
                Tissue::Flesh,
                false,
            )
            .with_functions(&[BodyPartFunction::Vision])
            .with_parent(ceph_id);
            body.add_part(eye);
            body.link_child(ceph_id, eye_id);
        }

        // Abdomen
        let abdomen_id = body.next_id();
        let abdomen = BodyPart::new(
            abdomen_id,
            "abdomen",
            BodyPartCategory::Torso,
            BodyPartSize::Medium,
            tissue,
            false,
        )
        .with_parent(ceph_id);
        body.add_part(abdomen);
        body.link_child(ceph_id, abdomen_id);

        // Eight legs
        for i in 1..=8 {
            let leg_id = body.next_id();
            let leg = BodyPart::new(
                leg_id,
                format!("leg {}", i),
                BodyPartCategory::LowerLimb,
                BodyPartSize::Small,
                tissue,
                false,
            )
            .with_functions(&[BodyPartFunction::Locomotion])
            .with_parent(ceph_id);
            body.add_part(leg);
            body.link_child(ceph_id, leg_id);
        }

        body
    }

    /// Create an insectoid body (scorpion)
    pub fn insectoid(tissue: Tissue) -> Self {
        let mut body = Body::new(BodyPlan::Insectoid, 35.0);

        // Cephalothorax
        let ceph_id = body.next_id();
        let ceph = BodyPart::new(
            ceph_id,
            "cephalothorax",
            BodyPartCategory::Torso,
            BodyPartSize::Medium,
            tissue,
            true,
        )
        .with_functions(&[BodyPartFunction::Breathing, BodyPartFunction::Thinking]);
        body.add_part(ceph);

        // Eyes
        for side in &["left", "right"] {
            let eye_id = body.next_id();
            let eye = BodyPart::new(
                eye_id,
                format!("{} eye", side),
                BodyPartCategory::Sensory,
                BodyPartSize::Tiny,
                Tissue::Flesh,
                false,
            )
            .with_functions(&[BodyPartFunction::Vision])
            .with_parent(ceph_id);
            body.add_part(eye);
            body.link_child(ceph_id, eye_id);
        }

        // Pincers
        for side in &["left", "right"] {
            let pincer_id = body.next_id();
            let pincer = BodyPart::new(
                pincer_id,
                format!("{} pincer", side),
                BodyPartCategory::UpperLimb,
                BodyPartSize::Medium,
                tissue,
                false,
            )
            .with_functions(&[BodyPartFunction::Grasping, BodyPartFunction::Attacking])
            .with_parent(ceph_id);
            body.add_part(pincer);
            body.link_child(ceph_id, pincer_id);
        }

        // Eight legs
        for i in 1..=8 {
            let leg_id = body.next_id();
            let leg = BodyPart::new(
                leg_id,
                format!("leg {}", i),
                BodyPartCategory::LowerLimb,
                BodyPartSize::Small,
                tissue,
                false,
            )
            .with_functions(&[BodyPartFunction::Locomotion])
            .with_parent(ceph_id);
            body.add_part(leg);
            body.link_child(ceph_id, leg_id);
        }

        // Tail with stinger
        let tail_id = body.next_id();
        let tail = BodyPart::new(
            tail_id,
            "tail",
            BodyPartCategory::Tail,
            BodyPartSize::Medium,
            tissue,
            false,
        )
        .with_functions(&[BodyPartFunction::Attacking])
        .with_parent(ceph_id);
        body.add_part(tail);
        body.link_child(ceph_id, tail_id);

        body
    }

    /// Create an avian body (griffin, phoenix)
    pub fn avian(tissue: Tissue) -> Self {
        let mut body = Body::new(BodyPlan::Avian, 60.0);

        // Torso
        let torso_id = body.next_id();
        let torso = BodyPart::new(
            torso_id,
            "torso",
            BodyPartCategory::Torso,
            BodyPartSize::Medium,
            tissue,
            true,
        )
        .with_functions(&[BodyPartFunction::Breathing]);
        body.add_part(torso);

        // Head
        let head_id = body.next_id();
        let head = BodyPart::new(
            head_id,
            "head",
            BodyPartCategory::Head,
            BodyPartSize::Small,
            tissue,
            true,
        )
        .with_functions(&[
            BodyPartFunction::Thinking,
            BodyPartFunction::Breathing,
            BodyPartFunction::Attacking,
        ])
        .with_parent(torso_id);
        body.add_part(head);
        body.link_child(torso_id, head_id);

        // Eyes
        for side in &["left", "right"] {
            let eye_id = body.next_id();
            let eye = BodyPart::new(
                eye_id,
                format!("{} eye", side),
                BodyPartCategory::Sensory,
                BodyPartSize::Tiny,
                Tissue::Flesh,
                false,
            )
            .with_functions(&[BodyPartFunction::Vision])
            .with_parent(head_id);
            body.add_part(eye);
            body.link_child(head_id, eye_id);
        }

        // Wings
        for side in &["left", "right"] {
            let wing_id = body.next_id();
            let wing = BodyPart::new(
                wing_id,
                format!("{} wing", side),
                BodyPartCategory::Wing,
                BodyPartSize::Large,
                tissue,
                false,
            )
            .with_functions(&[BodyPartFunction::Flight, BodyPartFunction::Attacking])
            .with_parent(torso_id);
            body.add_part(wing);
            body.link_child(torso_id, wing_id);
        }

        // Legs (for griffin - talons)
        for side in &["left", "right"] {
            let leg_id = body.next_id();
            let leg = BodyPart::new(
                leg_id,
                format!("{} leg", side),
                BodyPartCategory::LowerLimb,
                BodyPartSize::Medium,
                tissue,
                false,
            )
            .with_functions(&[BodyPartFunction::Locomotion, BodyPartFunction::Attacking])
            .with_parent(torso_id);
            body.add_part(leg);
            body.link_child(torso_id, leg_id);

            // Talons
            let talon_id = body.next_id();
            let talon = BodyPart::new(
                talon_id,
                format!("{} talons", side),
                BodyPartCategory::Extremity,
                BodyPartSize::Small,
                tissue,
                false,
            )
            .with_functions(&[BodyPartFunction::Attacking, BodyPartFunction::Grasping])
            .with_parent(leg_id);
            body.add_part(talon);
            body.link_child(leg_id, talon_id);
        }

        // Tail
        let tail_id = body.next_id();
        let tail = BodyPart::new(
            tail_id,
            "tail",
            BodyPartCategory::Tail,
            BodyPartSize::Small,
            tissue,
            false,
        )
        .with_functions(&[BodyPartFunction::Balance, BodyPartFunction::Flight])
        .with_parent(torso_id);
        body.add_part(tail);
        body.link_child(torso_id, tail_id);

        body
    }

    /// Create a serpentine body (hydra)
    pub fn serpentine(num_heads: u8, tissue: Tissue) -> Self {
        let mut body = Body::new(BodyPlan::Serpentine, 150.0 + (num_heads as f32 * 30.0));

        // Main body
        let body_id = body.next_id();
        let main_body = BodyPart::new(
            body_id,
            "body",
            BodyPartCategory::Torso,
            BodyPartSize::Huge,
            tissue,
            true,
        )
        .with_functions(&[BodyPartFunction::Breathing, BodyPartFunction::Locomotion]);
        body.add_part(main_body);

        // Multiple heads
        for i in 1..=num_heads {
            let neck_id = body.next_id();
            let neck = BodyPart::new(
                neck_id,
                format!("neck {}", i),
                BodyPartCategory::Special,
                BodyPartSize::Medium,
                tissue,
                false,
            )
            .with_parent(body_id);
            body.add_part(neck);
            body.link_child(body_id, neck_id);

            let head_id = body.next_id();
            let vital = i == 1; // Only first head is vital
            let head = BodyPart::new(
                head_id,
                format!("head {}", i),
                BodyPartCategory::Head,
                BodyPartSize::Medium,
                tissue,
                vital,
            )
            .with_functions(&[
                BodyPartFunction::Thinking,
                BodyPartFunction::Breathing,
                BodyPartFunction::Attacking,
            ])
            .with_parent(neck_id);
            body.add_part(head);
            body.link_child(neck_id, head_id);

            // Eyes for each head
            for side in &["left", "right"] {
                let eye_id = body.next_id();
                let eye = BodyPart::new(
                    eye_id,
                    format!("{} eye (head {})", side, i),
                    BodyPartCategory::Sensory,
                    BodyPartSize::Tiny,
                    Tissue::Flesh,
                    false,
                )
                .with_functions(&[BodyPartFunction::Vision])
                .with_parent(head_id);
                body.add_part(eye);
                body.link_child(head_id, eye_id);
            }
        }

        // Tail
        let tail_id = body.next_id();
        let tail = BodyPart::new(
            tail_id,
            "tail",
            BodyPartCategory::Tail,
            BodyPartSize::Large,
            tissue,
            false,
        )
        .with_functions(&[BodyPartFunction::Attacking, BodyPartFunction::Balance])
        .with_parent(body_id);
        body.add_part(tail);
        body.link_child(body_id, tail_id);

        body
    }

    /// Create a worm body (sandworm)
    pub fn worm(tissue: Tissue) -> Self {
        let mut body = Body::new(BodyPlan::Worm, 200.0);

        // Head segment
        let head_id = body.next_id();
        let head = BodyPart::new(
            head_id,
            "head",
            BodyPartCategory::Head,
            BodyPartSize::Large,
            tissue,
            true,
        )
        .with_functions(&[
            BodyPartFunction::Thinking,
            BodyPartFunction::Breathing,
            BodyPartFunction::Attacking,
        ]);
        body.add_part(head);

        // Body segments
        let mut prev_id = head_id;
        for i in 1..=5 {
            let segment_id = body.next_id();
            let segment = BodyPart::new(
                segment_id,
                format!("body segment {}", i),
                BodyPartCategory::Torso,
                BodyPartSize::Huge,
                tissue,
                false,
            )
            .with_functions(&[BodyPartFunction::Breathing, BodyPartFunction::Locomotion])
            .with_parent(prev_id);
            body.add_part(segment);
            body.link_child(prev_id, segment_id);
            prev_id = segment_id;
        }

        // Tail segment
        let tail_id = body.next_id();
        let tail = BodyPart::new(
            tail_id,
            "tail",
            BodyPartCategory::Tail,
            BodyPartSize::Large,
            tissue,
            false,
        )
        .with_functions(&[BodyPartFunction::Locomotion])
        .with_parent(prev_id);
        body.add_part(tail);
        body.link_child(prev_id, tail_id);

        body
    }

    /// Create a spectral body (bog wight)
    pub fn spectral() -> Self {
        let mut body = Body::new(BodyPlan::Spectral, 50.0);
        let tissue = Tissue::Spirit;

        // Ethereal form (single vital part)
        let form_id = body.next_id();
        let form = BodyPart::new(
            form_id,
            "ethereal form",
            BodyPartCategory::Torso,
            BodyPartSize::Medium,
            tissue,
            true,
        )
        .with_functions(&[
            BodyPartFunction::Thinking,
            BodyPartFunction::Attacking,
            BodyPartFunction::Locomotion,
        ]);
        body.add_part(form);

        // Spectral claws
        for side in &["left", "right"] {
            let claw_id = body.next_id();
            let claw = BodyPart::new(
                claw_id,
                format!("{} spectral claw", side),
                BodyPartCategory::Extremity,
                BodyPartSize::Small,
                tissue,
                false,
            )
            .with_functions(&[BodyPartFunction::Attacking])
            .with_parent(form_id);
            body.add_part(claw);
            body.link_child(form_id, claw_id);
        }

        body
    }

    // === Body query methods ===

    /// Get all targetable parts (not severed)
    pub fn targetable_parts(&self) -> Vec<&BodyPart> {
        self.parts
            .values()
            .filter(|p| !p.is_severed)
            .collect()
    }

    /// Get a part by ID
    pub fn get_part(&self, id: BodyPartId) -> Option<&BodyPart> {
        self.parts.get(&id)
    }

    /// Get a mutable part by ID
    pub fn get_part_mut(&mut self, id: BodyPartId) -> Option<&mut BodyPart> {
        self.parts.get_mut(&id)
    }

    /// Get all vital parts
    pub fn vital_parts(&self) -> Vec<&BodyPart> {
        self.parts.values().filter(|p| p.vital).collect()
    }

    /// Check if any vital part is destroyed
    pub fn is_dead(&self) -> bool {
        self.blood <= 0.0
            || self
                .parts
                .values()
                .any(|p| p.vital && (p.is_severed || p.health <= 0.0))
    }

    /// Get total impairment for a function (0.0 = fully functional, 1.0 = no function)
    pub fn function_impairment(&self, function: BodyPartFunction) -> f32 {
        let parts_with_function: Vec<_> = self
            .parts
            .values()
            .filter(|p| p.has_function(function))
            .collect();

        if parts_with_function.is_empty() {
            return 1.0; // No parts have this function
        }

        let total_impairment: f32 = parts_with_function.iter().map(|p| p.impairment()).sum();
        total_impairment / parts_with_function.len() as f32
    }

    /// Check if the body can perform a function
    pub fn can_perform(&self, function: BodyPartFunction) -> bool {
        self.function_impairment(function) < 1.0
    }

    /// Process bleeding for a tick
    pub fn process_bleeding(&mut self) {
        self.blood = (self.blood - self.blood_loss_rate).max(0.0);
    }

    /// Add to blood loss rate
    pub fn add_bleeding(&mut self, rate: f32) {
        self.blood_loss_rate += rate;
    }

    /// Reduce bleeding (from clotting or treatment)
    pub fn reduce_bleeding(&mut self, amount: f32) {
        self.blood_loss_rate = (self.blood_loss_rate - amount).max(0.0);
    }
}
