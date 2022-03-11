use crate::{MatchSpec, PackageRecord, Range, Version};
use itertools::Itertools;
use pubgrub::version_set::VersionSet;
use smallvec::SmallVec;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::iter::once;

/// A single AND group in a `MatchSpecConstraints`
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct MatchSpecElement {
    version: Range<Version>,
    build_number: Range<usize>,
}

impl MatchSpecElement {
    /// Returns an instance that matches nothing.
    fn none() -> Self {
        Self {
            version: Range::none(),
            build_number: Range::none(),
        }
    }

    /// Returns an instance that matches anything.
    fn any() -> Self {
        Self {
            version: Range::any(),
            build_number: Range::any(),
        }
    }

    /// Returns the intersection of this element and another
    fn intersection(&self, other: &Self) -> Self {
        let version = self.version.intersection(&other.version);
        let build_number = self.build_number.intersection(&other.build_number);
        if version == Range::none() || build_number == Range::none() {
            Self::none()
        } else {
            Self {
                version,
                build_number,
            }
        }
    }

    /// Returns true if the specified packages matches this instance
    pub fn contains(&self, package: &PackageRecord) -> bool {
        self.version.contains(&package.version) && self.build_number.contains(&package.build_number)
    }
}

/// Represents several constraints as a DNF.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct MatchSpecConstraints {
    groups: Vec<MatchSpecElement>,
}

impl From<MatchSpec> for MatchSpecConstraints {
    fn from(spec: MatchSpec) -> Self {
        Self {
            groups: vec![MatchSpecElement {
                version: spec.version.map(Into::into).unwrap_or_else(|| Range::any()),
                build_number: spec
                    .build_number
                    .clone()
                    .map(Range::equal)
                    .unwrap_or_else(|| Range::any()),
            }],
        }
    }
}

impl From<MatchSpecElement> for MatchSpecConstraints {
    fn from(elem: MatchSpecElement) -> Self {
        Self { groups: vec![elem] }
    }
}

impl Display for MatchSpecConstraints {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "bla")
    }
}

impl VersionSet for MatchSpecConstraints {
    type V = PackageRecord;

    fn empty() -> Self {
        Self { groups: vec![] }
    }

    fn full() -> Self {
        Self {
            groups: vec![MatchSpecElement {
                version: Range::any(),
                build_number: Range::any(),
            }],
        }
    }

    fn singleton(v: Self::V) -> Self {
        Self {
            groups: vec![MatchSpecElement {
                version: Range::equal(v.version),
                build_number: Range::equal(v.build_number),
            }],
        }
    }

    fn complement(&self) -> Self {
        if self.groups.is_empty() {
            Self {
                groups: vec![MatchSpecElement::any()],
            }
        } else {
            let mut permutations = Vec::with_capacity(self.groups.len());
            for spec in self.groups.iter() {
                let mut group_entries: SmallVec<[MatchSpecElement; 2]> = SmallVec::new();
                let version_complement = spec.version.negate();
                if version_complement != Range::none() {
                    group_entries.push(MatchSpecElement {
                        version: version_complement,
                        build_number: Range::any(),
                    });
                }

                let build_complement = spec.build_number.negate();
                if build_complement != Range::none() {
                    group_entries.push(MatchSpecElement {
                        version: Range::any(),
                        build_number: spec.build_number.negate(),
                    });
                }

                permutations.push(group_entries);
            }

            let mut groups = HashSet::new();
            for perm in permutations.into_iter().multi_cartesian_product() {
                let group = perm.into_iter().reduce(|a, b| a.intersection(&b)).unwrap();

                if group == MatchSpecElement::any() {
                    return MatchSpecConstraints::from(group);
                } else if group != MatchSpecElement::none() {
                    groups.insert(group);
                }
            }

            Self {
                groups: groups
                    .into_iter()
                    .sorted_by_cached_key(|e| {
                        let mut hasher = DefaultHasher::new();
                        e.hash(&mut hasher);
                        hasher.finish()
                    })
                    .collect(),
            }
        }
    }

    fn intersection(&self, other: &Self) -> Self {
        let mut groups = once(self.groups.iter())
            .chain(once(other.groups.iter()))
            .multi_cartesian_product()
            .map(|elems| {
                elems
                    .into_iter()
                    .cloned()
                    .reduce(|a, b| a.intersection(&b))
                    .unwrap()
            })
            .filter(|group| group != &MatchSpecElement::none())
            .collect_vec();

        if groups.iter().any(|group| group == &MatchSpecElement::any()) {
            return MatchSpecElement::any().into();
        }

        groups.sort_by_cached_key(|e| {
            let mut hasher = DefaultHasher::new();
            e.hash(&mut hasher);
            hasher.finish()
        });

        Self { groups }
    }

    fn contains(&self, v: &Self::V) -> bool {
        self.groups.iter().any(|group| group.contains(v))
    }
}

#[cfg(test)]
mod tests {
    use crate::match_spec_constraints::MatchSpecConstraints;
    use crate::{PackageRecord, Version};
    use pubgrub::version_set::VersionSet;
    use std::str::FromStr;

    #[test]
    fn complement() {
        let record = PackageRecord {
            name: "".to_string(),
            version: Version::from_str("1.2.3").unwrap(),
            build: "".to_string(),
            build_number: 1,
            subdir: "".to_string(),
            md5: None,
            sha256: None,
            arch: None,
            platform: None,
            depends: vec![],
            constrains: vec![],
            track_features: None,
            features: None,
            preferred_env: None,
            license: None,
            license_family: None,
            timestamp: None,
            date: None,
            size: None,
        };

        let constraint = MatchSpecConstraints::singleton(record.clone());

        assert!(constraint.contains(&record));
        assert!(!constraint.complement().contains(&record));

        assert_eq!(constraint.intersection(&constraint), constraint);
        assert_eq!(
            constraint.intersection(&constraint.complement()),
            MatchSpecConstraints::empty()
        );

        assert_eq!(
            constraint
                .complement()
                .complement()
                .complement()
                .complement(),
            constraint
        );
        assert_eq!(
            constraint.complement().complement().complement(),
            constraint.complement()
        );

        assert_eq!(
            MatchSpecConstraints::empty(),
            constraint.complement().intersection(&constraint)
        );
        assert_eq!(
            MatchSpecConstraints::full(),
            constraint.complement().union(&constraint)
        );
    }
}
