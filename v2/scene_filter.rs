// The gameplay loader skips presentation entities. Keep global effects separately.
pub fn gameplay(k: &str) -> bool {
    !matches!(
        k,
        "Animating"
            | "Audio_Marker"
            | "Camera_Control"
            | "Camera_Marker"
            | "Decal"
            | "Entity_Issued_Sound"
            | "Flat_Decoration"
            | "Fog"
            | "Fog_Of_War"
            | "Fog_Of_War_Clear"
            | "Fog_Of_War_Z"
            | "Fog_Volume"
            | "Global_Effects_Settings"
            | "Inanimate"
            | "Light_Probe"
            | "Outline_Override"
            | "Particle_Force"
            | "Particle_Source"
            | "Per_Screen_Audio"
            | "Playtest_Note"
            | "Point_Light"
            | "Procedurally_Animating"
            | "Ripples"
            | "Shadow_Decal"
            | "Sky"
            | "Sun"
            | "Temporary_Splash"
            | "Translucent_Shadow"
            | "Turbulence"
            | "View_Giver"
            | "Voice_Log"
            | "Wind"
    )
}

// Creatures select their graph by family/flags rather than by mesh name.
pub const CREATURE_GRAPHS: [&str; 4] =
    ["promAdultDragon", "promBabyDragon", "goblin", "sokodragon"];
