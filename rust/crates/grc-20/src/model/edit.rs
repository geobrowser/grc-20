//! Edit structure for batched operations.
//!
//! Edits are standalone patches containing a batch of ops with metadata.

use std::borrow::Cow;

use rustc_hash::FxHashMap;

use crate::codec::primitives::Writer;
use crate::model::{DataType, Id, Op};

/// A batch of operations with metadata (spec Section 4.1).
///
/// Edits are standalone patches. They contain no parent references;
/// ordering is provided by on-chain governance.
#[derive(Debug, Clone, PartialEq)]
pub struct Edit<'a> {
    /// The edit's unique identifier.
    pub id: Id,
    /// Optional human-readable name.
    pub name: Cow<'a, str>,
    /// Author entity IDs.
    pub authors: Vec<Id>,
    /// Creation timestamp (metadata only, not used for conflict resolution).
    pub created_at: i64,
    /// Operations in this edit.
    pub ops: Vec<Op<'a>>,
}

impl<'a> Edit<'a> {
    /// Creates a new empty edit with the given ID.
    pub fn new(id: Id) -> Self {
        Self {
            id,
            name: Cow::Borrowed(""),
            authors: Vec::new(),
            created_at: 0,
            ops: Vec::new(),
        }
    }

    /// Creates a new empty edit with the given ID and name.
    pub fn with_name(id: Id, name: impl Into<Cow<'a, str>>) -> Self {
        Self {
            id,
            name: name.into(),
            authors: Vec::new(),
            created_at: 0,
            ops: Vec::new(),
        }
    }
}

/// Wire-format dictionaries for encoding/decoding.
///
/// These dictionaries map between full IDs and compact indices
/// within an edit.
#[derive(Debug, Clone, Default)]
pub struct WireDictionaries {
    /// Properties dictionary: (ID, DataType) pairs.
    pub properties: Vec<(Id, DataType)>,
    /// Relation type IDs.
    pub relation_types: Vec<Id>,
    /// Language entity IDs for localized TEXT values.
    pub languages: Vec<Id>,
    /// Unit entity IDs for numerical values.
    pub units: Vec<Id>,
    /// Object IDs (entities and relations).
    pub objects: Vec<Id>,
}

impl WireDictionaries {
    /// Creates empty dictionaries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks up a property ID by index.
    pub fn get_property(&self, index: usize) -> Option<&(Id, DataType)> {
        self.properties.get(index)
    }

    /// Looks up a relation type ID by index.
    pub fn get_relation_type(&self, index: usize) -> Option<&Id> {
        self.relation_types.get(index)
    }

    /// Looks up a language ID by index.
    ///
    /// Index 0 means default (no language), returns None.
    /// Index 1+ maps to languages[index-1].
    pub fn get_language(&self, index: usize) -> Option<&Id> {
        if index == 0 {
            None
        } else {
            self.languages.get(index - 1)
        }
    }

    /// Looks up a unit ID by index.
    ///
    /// Index 0 means no unit, returns None.
    /// Index 1+ maps to units[index-1].
    pub fn get_unit(&self, index: usize) -> Option<&Id> {
        if index == 0 {
            None
        } else {
            self.units.get(index - 1)
        }
    }

    /// Looks up an object ID by index.
    pub fn get_object(&self, index: usize) -> Option<&Id> {
        self.objects.get(index)
    }
}

/// Builder for constructing wire dictionaries during encoding.
///
/// Uses FxHashMap for faster hashing of 16-byte IDs.
#[derive(Debug, Clone, Default)]
pub struct DictionaryBuilder {
    properties: Vec<(Id, DataType)>,
    property_indices: FxHashMap<Id, usize>,
    relation_types: Vec<Id>,
    relation_type_indices: FxHashMap<Id, usize>,
    languages: Vec<Id>,
    language_indices: FxHashMap<Id, usize>,
    units: Vec<Id>,
    unit_indices: FxHashMap<Id, usize>,
    objects: Vec<Id>,
    object_indices: FxHashMap<Id, usize>,
}

impl DictionaryBuilder {
    /// Creates a new empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new builder with pre-allocated capacity.
    ///
    /// `estimated_ops` is used to estimate dictionary sizes:
    /// - properties: ~estimated_ops / 4 (entities average ~4 properties)
    /// - relation_types: ~estimated_ops / 20 (fewer unique relation types)
    /// - languages: 4 (typically few languages per edit)
    /// - units: 4 (typically few units per edit)
    /// - objects: ~estimated_ops / 2 (many ops reference existing objects)
    pub fn with_capacity(estimated_ops: usize) -> Self {
        let prop_cap = estimated_ops / 4 + 1;
        let rel_cap = estimated_ops / 20 + 1;
        let lang_cap = 4;
        let unit_cap = 4;
        let obj_cap = estimated_ops / 2 + 1;

        Self {
            properties: Vec::with_capacity(prop_cap),
            property_indices: FxHashMap::with_capacity_and_hasher(prop_cap, Default::default()),
            relation_types: Vec::with_capacity(rel_cap),
            relation_type_indices: FxHashMap::with_capacity_and_hasher(rel_cap, Default::default()),
            languages: Vec::with_capacity(lang_cap),
            language_indices: FxHashMap::with_capacity_and_hasher(lang_cap, Default::default()),
            units: Vec::with_capacity(unit_cap),
            unit_indices: FxHashMap::with_capacity_and_hasher(unit_cap, Default::default()),
            objects: Vec::with_capacity(obj_cap),
            object_indices: FxHashMap::with_capacity_and_hasher(obj_cap, Default::default()),
        }
    }

    /// Adds or gets the index for a property.
    pub fn add_property(&mut self, id: Id, data_type: DataType) -> usize {
        if let Some(&idx) = self.property_indices.get(&id) {
            idx
        } else {
            let idx = self.properties.len();
            self.properties.push((id, data_type));
            self.property_indices.insert(id, idx);
            idx
        }
    }

    /// Adds or gets the index for a relation type.
    pub fn add_relation_type(&mut self, id: Id) -> usize {
        if let Some(&idx) = self.relation_type_indices.get(&id) {
            idx
        } else {
            let idx = self.relation_types.len();
            self.relation_types.push(id);
            self.relation_type_indices.insert(id, idx);
            idx
        }
    }

    /// Adds or gets the index for a language.
    ///
    /// Returns 0 for default (no language), 1+ for actual languages.
    pub fn add_language(&mut self, id: Option<Id>) -> usize {
        match id {
            None => 0,
            Some(lang_id) => {
                if let Some(&idx) = self.language_indices.get(&lang_id) {
                    idx + 1
                } else {
                    let idx = self.languages.len();
                    self.languages.push(lang_id);
                    self.language_indices.insert(lang_id, idx);
                    idx + 1
                }
            }
        }
    }

    /// Adds or gets the index for a unit.
    ///
    /// Returns 0 for no unit, 1+ for actual units.
    pub fn add_unit(&mut self, id: Option<Id>) -> usize {
        match id {
            None => 0,
            Some(unit_id) => {
                if let Some(&idx) = self.unit_indices.get(&unit_id) {
                    idx + 1
                } else {
                    let idx = self.units.len();
                    self.units.push(unit_id);
                    self.unit_indices.insert(unit_id, idx);
                    idx + 1
                }
            }
        }
    }

    /// Adds or gets the index for an object.
    pub fn add_object(&mut self, id: Id) -> usize {
        if let Some(&idx) = self.object_indices.get(&id) {
            idx
        } else {
            let idx = self.objects.len();
            self.objects.push(id);
            self.object_indices.insert(id, idx);
            idx
        }
    }

    /// Builds the final wire dictionaries (consumes the builder).
    pub fn build(self) -> WireDictionaries {
        WireDictionaries {
            properties: self.properties,
            relation_types: self.relation_types,
            languages: self.languages,
            units: self.units,
            objects: self.objects,
        }
    }

    /// Returns a reference to wire dictionaries without consuming the builder.
    /// This allows continued use of the builder for encoding while having the dictionaries.
    pub fn as_wire_dicts(&self) -> WireDictionaries {
        WireDictionaries {
            properties: self.properties.clone(),
            relation_types: self.relation_types.clone(),
            languages: self.languages.clone(),
            units: self.units.clone(),
            objects: self.objects.clone(),
        }
    }

    /// Gets the index for an existing property (for encoding).
    pub fn get_property_index(&self, id: &Id) -> Option<usize> {
        self.property_indices.get(id).copied()
    }

    /// Gets the index for an existing relation type (for encoding).
    pub fn get_relation_type_index(&self, id: &Id) -> Option<usize> {
        self.relation_type_indices.get(id).copied()
    }

    /// Gets the index for an existing language (for encoding).
    /// Returns 0 for None, 1+ for existing languages.
    pub fn get_language_index(&self, id: Option<&Id>) -> Option<usize> {
        match id {
            None => Some(0),
            Some(lang_id) => self.language_indices.get(lang_id).map(|idx| idx + 1),
        }
    }

    /// Gets the index for an existing object (for encoding).
    pub fn get_object_index(&self, id: &Id) -> Option<usize> {
        self.object_indices.get(id).copied()
    }

    /// Writes the dictionaries directly to a writer (avoids cloning).
    pub fn write_dictionaries(&self, writer: &mut Writer) {
        // Properties: count + (id, data_type) pairs
        writer.write_varint(self.properties.len() as u64);
        for (id, data_type) in &self.properties {
            writer.write_id(id);
            writer.write_byte(*data_type as u8);
        }

        // Relation types
        writer.write_id_vec(&self.relation_types);

        // Languages
        writer.write_id_vec(&self.languages);

        // Units
        writer.write_id_vec(&self.units);

        // Objects
        writer.write_id_vec(&self.objects);
    }

    /// Converts this builder into a sorted canonical form.
    ///
    /// All dictionaries are sorted by ID bytes (lexicographic order),
    /// and the index maps are rebuilt to reflect the new ordering.
    ///
    /// This is used for canonical encoding to ensure deterministic output.
    pub fn into_sorted(self) -> Self {
        // Sort properties by ID
        let mut properties = self.properties;
        properties.sort_by(|a, b| a.0.cmp(&b.0));
        let property_indices: FxHashMap<Id, usize> = properties
            .iter()
            .enumerate()
            .map(|(i, (id, _))| (*id, i))
            .collect();

        // Sort relation types by ID
        let mut relation_types = self.relation_types;
        relation_types.sort();
        let relation_type_indices: FxHashMap<Id, usize> = relation_types
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect();

        // Sort languages by ID
        let mut languages = self.languages;
        languages.sort();
        let language_indices: FxHashMap<Id, usize> = languages
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect();

        // Sort units by ID
        let mut units = self.units;
        units.sort();
        let unit_indices: FxHashMap<Id, usize> = units
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect();

        // Sort objects by ID
        let mut objects = self.objects;
        objects.sort();
        let object_indices: FxHashMap<Id, usize> = objects
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect();

        Self {
            properties,
            property_indices,
            relation_types,
            relation_type_indices,
            languages,
            language_indices,
            units,
            unit_indices,
            objects,
            object_indices,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_new() {
        let id = [1u8; 16];
        let edit = Edit::new(id);
        assert_eq!(edit.id, id);
        assert!(edit.name.is_empty());
        assert!(edit.authors.is_empty());
        assert!(edit.ops.is_empty());
    }

    #[test]
    fn test_dictionary_builder() {
        let mut builder = DictionaryBuilder::new();

        let prop1 = [1u8; 16];
        let prop2 = [2u8; 16];

        // First add returns 0
        assert_eq!(builder.add_property(prop1, DataType::Text), 0);
        // Second add of same ID returns same index
        assert_eq!(builder.add_property(prop1, DataType::Text), 0);
        // Different ID gets new index
        assert_eq!(builder.add_property(prop2, DataType::Int64), 1);

        let dicts = builder.build();
        assert_eq!(dicts.properties.len(), 2);
        assert_eq!(dicts.properties[0], (prop1, DataType::Text));
        assert_eq!(dicts.properties[1], (prop2, DataType::Int64));
    }

    #[test]
    fn test_language_indexing() {
        let mut builder = DictionaryBuilder::new();

        let lang1 = [10u8; 16];
        let lang2 = [20u8; 16];

        // None returns 0
        assert_eq!(builder.add_language(None), 0);
        // First language returns 1
        assert_eq!(builder.add_language(Some(lang1)), 1);
        // Same language returns same index
        assert_eq!(builder.add_language(Some(lang1)), 1);
        // Different language returns 2
        assert_eq!(builder.add_language(Some(lang2)), 2);

        let dicts = builder.build();
        assert_eq!(dicts.languages.len(), 2);

        // get_language(0) returns None (default)
        assert!(dicts.get_language(0).is_none());
        // get_language(1) returns lang1
        assert_eq!(dicts.get_language(1), Some(&lang1));
        // get_language(2) returns lang2
        assert_eq!(dicts.get_language(2), Some(&lang2));
    }
}
