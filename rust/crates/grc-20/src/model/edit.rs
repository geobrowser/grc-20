//! Edit structure for batched operations.
//!
//! Edits are standalone patches containing a batch of ops with metadata.

use std::borrow::Cow;

use rustc_hash::FxHashMap;

use crate::codec::primitives::Writer;
use crate::error::EncodeError;
use crate::limits::MAX_DICT_SIZE;
use crate::model::{DataType, Id, Op};

/// An edge in a context path (spec Section 4.5).
///
/// Represents a step in the path from the root entity to the changed entity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContextEdge {
    /// The relation type ID for this edge (e.g., BLOCKS_ID).
    pub type_id: Id,
    /// The target entity ID at this edge.
    pub to_entity_id: Id,
}

/// Context metadata for grouping changes (spec Section 4.5).
///
/// Provides the path from a root entity to the changed entity,
/// enabling context-aware change grouping (e.g., grouping block changes
/// under their parent entity).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Context {
    /// The root entity for this context.
    pub root_id: Id,
    /// Path from root to the changed entity.
    pub edges: Vec<ContextEdge>,
}

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
    /// Context IDs (root_ids and edge to_entity_ids) - used during encoding/decoding.
    pub context_ids: Vec<Id>,
    /// Decoded contexts array - used by op decoders to resolve context_ref to Context.
    pub contexts: Vec<Context>,
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

    /// Looks up a context ID by index.
    pub fn get_context_id(&self, index: usize) -> Option<&Id> {
        self.context_ids.get(index)
    }

    /// Looks up a context by index.
    pub fn get_context(&self, index: usize) -> Option<&Context> {
        self.contexts.get(index)
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
    context_ids: Vec<Id>,
    context_id_indices: FxHashMap<Id, usize>,
    contexts: Vec<Context>,
    context_indices: FxHashMap<Context, usize>,
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
    /// - context_ids: 8 (typically few context IDs per edit)
    /// - contexts: 4 (typically few unique contexts per edit)
    pub fn with_capacity(estimated_ops: usize) -> Self {
        let prop_cap = estimated_ops / 4 + 1;
        let rel_cap = estimated_ops / 20 + 1;
        let lang_cap = 4;
        let unit_cap = 4;
        let obj_cap = estimated_ops / 2 + 1;
        let ctx_id_cap = 8;
        let ctx_cap = 4;

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
            context_ids: Vec::with_capacity(ctx_id_cap),
            context_id_indices: FxHashMap::with_capacity_and_hasher(ctx_id_cap, Default::default()),
            contexts: Vec::with_capacity(ctx_cap),
            context_indices: FxHashMap::with_capacity_and_hasher(ctx_cap, Default::default()),
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

    /// Adds or gets the index for a context ID.
    pub fn add_context_id(&mut self, id: Id) -> usize {
        if let Some(&idx) = self.context_id_indices.get(&id) {
            idx
        } else {
            let idx = self.context_ids.len();
            self.context_ids.push(id);
            self.context_id_indices.insert(id, idx);
            idx
        }
    }

    /// Adds or gets the index for a context.
    ///
    /// If the context is new, registers all its IDs to the appropriate dictionaries:
    /// - root_id and edge.to_entity_id go to context_ids dictionary
    /// - edge.type_id goes to relation_types dictionary (it's a RelationTypeRef)
    /// Returns the index into the contexts array.
    pub fn add_context(&mut self, context: &Context) -> usize {
        if let Some(&idx) = self.context_indices.get(context) {
            idx
        } else {
            // Register all IDs in the context to appropriate dictionaries
            self.add_context_id(context.root_id);
            for edge in &context.edges {
                // type_id is a relation type, not a context ID
                self.add_relation_type(edge.type_id);
                self.add_context_id(edge.to_entity_id);
            }

            // Add context to contexts array
            let idx = self.contexts.len();
            self.contexts.push(context.clone());
            self.context_indices.insert(context.clone(), idx);
            idx
        }
    }

    /// Gets the index for an existing context (for encoding).
    pub fn get_context_index(&self, context: &Context) -> Option<usize> {
        self.context_indices.get(context).copied()
    }

    /// Builds the final wire dictionaries (consumes the builder).
    pub fn build(self) -> WireDictionaries {
        WireDictionaries {
            properties: self.properties,
            relation_types: self.relation_types,
            languages: self.languages,
            units: self.units,
            objects: self.objects,
            context_ids: self.context_ids,
            contexts: self.contexts,
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
            context_ids: self.context_ids.clone(),
            contexts: self.contexts.clone(),
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

    /// Gets the index for an existing context ID (for encoding).
    pub fn get_context_id_index(&self, id: &Id) -> Option<usize> {
        self.context_id_indices.get(id).copied()
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

        // Context IDs
        writer.write_id_vec(&self.context_ids);
    }

    /// Writes the contexts array to the writer.
    ///
    /// Each context is encoded as:
    /// - root_id: varint (index into context_ids)
    /// - edge_count: varint
    /// - edges: for each edge: type_id (RelationTypeRef), to_entity_id (ContextRef)
    pub fn write_contexts(&self, writer: &mut Writer) {
        writer.write_varint(self.contexts.len() as u64);
        for ctx in &self.contexts {
            // Root ID as context_id index
            let root_idx = self.context_id_indices.get(&ctx.root_id)
                .copied()
                .expect("context root_id must be in context_ids dictionary");
            writer.write_varint(root_idx as u64);

            // Edges
            writer.write_varint(ctx.edges.len() as u64);
            for edge in &ctx.edges {
                // type_id is a RelationTypeRef (index into relation_types dictionary)
                let type_idx = self.relation_type_indices.get(&edge.type_id)
                    .copied()
                    .expect("context edge type_id must be in relation_types dictionary");
                // to_entity_id is a ContextRef (index into context_ids dictionary)
                let to_idx = self.context_id_indices.get(&edge.to_entity_id)
                    .copied()
                    .expect("context edge to_entity_id must be in context_ids dictionary");
                writer.write_varint(type_idx as u64);
                writer.write_varint(to_idx as u64);
            }
        }
    }

    /// Validates dictionary and context sizes against codec limits.
    pub fn validate_limits(&self) -> Result<(), EncodeError> {
        let max = MAX_DICT_SIZE;
        if self.properties.len() > max {
            return Err(EncodeError::LengthExceedsLimit {
                field: "properties",
                len: self.properties.len(),
                max,
            });
        }
        if self.relation_types.len() > max {
            return Err(EncodeError::LengthExceedsLimit {
                field: "relation_types",
                len: self.relation_types.len(),
                max,
            });
        }
        if self.languages.len() > max {
            return Err(EncodeError::LengthExceedsLimit {
                field: "languages",
                len: self.languages.len(),
                max,
            });
        }
        if self.units.len() > max {
            return Err(EncodeError::LengthExceedsLimit {
                field: "units",
                len: self.units.len(),
                max,
            });
        }
        if self.objects.len() > max {
            return Err(EncodeError::LengthExceedsLimit {
                field: "objects",
                len: self.objects.len(),
                max,
            });
        }
        if self.context_ids.len() > max {
            return Err(EncodeError::LengthExceedsLimit {
                field: "context_ids",
                len: self.context_ids.len(),
                max,
            });
        }
        if self.contexts.len() > max {
            return Err(EncodeError::LengthExceedsLimit {
                field: "contexts",
                len: self.contexts.len(),
                max,
            });
        }
        for ctx in &self.contexts {
            if ctx.edges.len() > max {
                return Err(EncodeError::LengthExceedsLimit {
                    field: "context_edges",
                    len: ctx.edges.len(),
                    max,
                });
            }
        }
        Ok(())
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

        // Sort context IDs by ID
        let mut context_ids = self.context_ids;
        context_ids.sort();
        let context_id_indices: FxHashMap<Id, usize> = context_ids
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect();

        // Sort contexts by root_id, then by edges (canonically)
        let mut contexts = self.contexts;
        contexts.sort_by(|a, b| {
            // First compare by root_id
            match a.root_id.cmp(&b.root_id) {
                std::cmp::Ordering::Equal => {
                    // Then compare edges lexicographically
                    let a_edges: Vec<_> = a.edges.iter().map(|e| (e.type_id, e.to_entity_id)).collect();
                    let b_edges: Vec<_> = b.edges.iter().map(|e| (e.type_id, e.to_entity_id)).collect();
                    a_edges.cmp(&b_edges)
                }
                other => other,
            }
        });
        let context_indices: FxHashMap<Context, usize> = contexts
            .iter()
            .enumerate()
            .map(|(i, ctx)| (ctx.clone(), i))
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
            context_ids,
            context_id_indices,
            contexts,
            context_indices,
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
