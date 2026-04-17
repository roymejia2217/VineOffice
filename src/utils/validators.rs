use which::which;

pub struct DependencyValidator;

impl DependencyValidator {
    pub fn check_wine() -> bool {
        which("wine").is_ok()
    }

    pub fn check_winetricks() -> bool {
        which("winetricks").is_ok()
    }

    pub fn check_cabextract() -> bool {
        which("cabextract").is_ok()
    }

    pub fn check_winbind() -> bool {
        which("winbindd").is_ok() || which("nmblookup").is_ok()
    }

    pub fn check_all() -> Vec<(&'static str, bool)> {
        vec![
            ("wine", Self::check_wine()),
            ("winetricks", Self::check_winetricks()),
            ("cabextract", Self::check_cabextract()),
            ("winbind", Self::check_winbind()),
        ]
    }

    pub fn get_missing_dependencies() -> Vec<&'static str> {
        Self::check_all()
            .into_iter()
            .filter(|(_, ok)| !ok)
            .map(|(name, _)| name)
            .collect()
    }
}
