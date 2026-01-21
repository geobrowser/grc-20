import type { Id } from "./id.js";
import type { Op } from "./op.js";
import type { DataType } from "./value.js";

/**
 * An edge in a context path (spec Section 4.5).
 *
 * Represents a step in the path from the root entity to the changed entity.
 */
export interface ContextEdge {
  /** The relation type ID for this edge (e.g., BLOCKS_ID). */
  typeId: Id;
  /** The target entity ID at this edge. */
  toEntityId: Id;
}

/**
 * Context metadata for grouping changes (spec Section 4.5).
 *
 * Provides the path from a root entity to the changed entity,
 * enabling context-aware change grouping (e.g., grouping block changes
 * under their parent entity).
 */
export interface Context {
  /** The root entity for this context. */
  rootId: Id;
  /** Path from root to the changed entity. */
  edges: ContextEdge[];
}

/**
 * A batch of operations with metadata (spec Section 4.1).
 *
 * Edits are standalone patches. They contain no parent references;
 * ordering is provided by on-chain governance.
 */
export interface Edit {
  /** The edit's unique identifier. */
  id: Id;
  /** Optional human-readable name (may be empty string). */
  name: string;
  /** Author entity IDs. */
  authors: Id[];
  /** Creation timestamp in microseconds since Unix epoch (metadata only). */
  createdAt: bigint;
  /** Operations in this edit. */
  ops: Op[];
}

/**
 * Wire-format dictionaries for encoding/decoding.
 *
 * These dictionaries map between full IDs and compact indices within an edit.
 */
export interface WireDictionaries {
  /** Properties dictionary: (ID, DataType) pairs. */
  properties: Array<{ id: Id; dataType: DataType }>;
  /** Relation type IDs. */
  relationTypes: Id[];
  /** Language entity IDs for localized TEXT values. */
  languages: Id[];
  /** Unit entity IDs for numerical values. */
  units: Id[];
  /** Object IDs (entities and relations). */
  objects: Id[];
  /** Context IDs (root_ids and edge to_entity_ids). */
  contextIds: Id[];
  /** Decoded contexts array - used by op decoders to resolve context_ref to Context. */
  contexts: Context[];
}

/**
 * Creates empty wire dictionaries.
 */
export function createWireDictionaries(): WireDictionaries {
  return {
    properties: [],
    relationTypes: [],
    languages: [],
    units: [],
    objects: [],
    contextIds: [],
    contexts: [],
  };
}
