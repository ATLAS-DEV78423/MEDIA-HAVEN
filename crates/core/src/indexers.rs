//! Provider definitions bundled into the binary for one-click Prowlarr seeding.

/// A Prowlarr indexer definition embedded into the binary.
pub struct BundledIndexer {
    /// Prowlarr definition id.
    pub id: &'static str,
    /// Display name from the definition file.
    pub name: &'static str,
    /// Source file name under `resources/indexers/`.
    pub definition_file: &'static str,
    /// Verbatim YAML contents of the definition file.
    pub yaml: &'static str,
}

/// The five provider definitions shipped with MEDIA_HAVEN.
pub const BUNDLED_INDEXERS: &[BundledIndexer] = &[
    BundledIndexer {
        id: "yts",
        name: "YTS",
        definition_file: "yts.yml",
        yaml: include_str!("../resources/indexers/yts.yml"),
    },
    BundledIndexer {
        id: "nyaasi",
        name: "Nyaa.si",
        definition_file: "nyaasi.yml",
        yaml: include_str!("../resources/indexers/nyaasi.yml"),
    },
    BundledIndexer {
        id: "thepiratebay",
        name: "The Pirate Bay",
        definition_file: "thepiratebay.yml",
        yaml: include_str!("../resources/indexers/thepiratebay.yml"),
    },
    BundledIndexer {
        id: "eztv-api",
        name: "EZTV (API)",
        definition_file: "eztv-api.yml",
        yaml: include_str!("../resources/indexers/eztv-api.yml"),
    },
    BundledIndexer {
        id: "animetosho",
        name: "AnimeTosho",
        definition_file: "animetosho.yml",
        yaml: include_str!("../resources/indexers/animetosho.yml"),
    },
];

#[cfg(test)]
mod tests {
    use super::BUNDLED_INDEXERS;

    #[test]
    fn bundles_five_valid_definitions() {
        assert_eq!(BUNDLED_INDEXERS.len(), 5);
        for entry in BUNDLED_INDEXERS {
            assert!(entry.yaml.starts_with("---"), "{}", entry.id);
            assert!(
                entry.yaml.lines().any(|line| line.starts_with("id:")),
                "{}",
                entry.id
            );
        }
    }
}
