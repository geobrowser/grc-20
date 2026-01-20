import type { Id } from "../types/id.js";
import type { Edit } from "../types/edit.js";
import type {
  CreateEntity,
  CreateRelation,
  CreateValueRef,
  DeleteEntity,
  DeleteRelation,
  Op,
  RestoreEntity,
  RestoreRelation,
  UnsetRelationField,
  UnsetValue,
  UpdateEntity,
  UpdateRelation,
} from "../types/op.js";
import type { PropertyValue } from "../types/value.js";
import { randomId } from "../util/id.js";

/**
 * Input for creating an Edit.
 */
export interface CreateEditInput {
  /** The edit ID. If omitted, a random ID is generated. */
  id?: Id;
  /** Human-readable name for the edit. */
  name?: string;
  /** Single author ID (convenience for single-author edits). */
  author?: Id;
  /** Multiple author IDs. Takes precedence over `author` if both provided. */
  authors?: Id[];
  /** Creation timestamp in microseconds since Unix epoch. */
  createdAt?: bigint;
  /** Operations to include in the edit. */
  ops?: Op[];
}

/**
 * Creates an Edit from the given input.
 *
 * This is the functional API equivalent to using {@link EditBuilder}.
 *
 * @param input - The edit configuration
 * @returns A new Edit object
 *
 * @example
 * ```ts
 * const edit = createEdit({
 *   id: myEditId,
 *   name: "Add new entity",
 *   author: authorId,
 *   ops: [createEntity({ id: entityId, values: [...] })],
 * });
 * ```
 */
export function createEdit(input: CreateEditInput): Edit {
  const authors = input.authors ? [...input.authors] : input.author ? [input.author] : [];

  return {
    id: input.id ?? randomId(),
    name: input.name ?? "",
    authors,
    createdAt: input.createdAt ?? 0n,
    ops: input.ops ? [...input.ops] : [],
  };
}

/**
 * Input for creating a CreateEntity operation.
 */
export interface CreateEntityInput {
  /** The entity ID. */
  id: Id;
  /** Property values to set on the entity. */
  values?: PropertyValue[];
}

/**
 * Creates a CreateEntity operation.
 *
 * If the entity does not exist, it will be created. If it already exists,
 * this acts as an update: values are applied as set_properties (LWW).
 *
 * @param input - The entity configuration
 * @returns A CreateEntity operation
 *
 * @example
 * ```ts
 * const op = createEntity({
 *   id: entityId,
 *   values: [
 *     { property: properties.name(), value: { type: "text", value: "Alice" } },
 *   ],
 * });
 * ```
 */
export function createEntity(input: CreateEntityInput): CreateEntity {
  return {
    type: "createEntity",
    id: input.id,
    values: input.values ?? [],
  };
}

/**
 * Input for creating an UpdateEntity operation.
 */
export interface UpdateEntityInput {
  /** The entity ID to update. */
  id: Id;
  /** Property values to set (last-writer-wins). */
  set?: PropertyValue[];
  /** Property values to unset/clear. */
  unset?: UnsetValue[];
}

/**
 * Creates an UpdateEntity operation.
 *
 * Updates an existing entity by setting and/or unsetting property values.
 * Set operations use last-writer-wins (LWW) semantics.
 *
 * @param input - The update configuration
 * @returns An UpdateEntity operation
 *
 * @example
 * ```ts
 * const op = updateEntity({
 *   id: entityId,
 *   set: [{ property: propId, value: { type: "text", value: "New value" } }],
 *   unset: [{ property: oldPropId, language: { type: "all" } }],
 * });
 * ```
 */
export function updateEntity(input: UpdateEntityInput): UpdateEntity {
  return {
    type: "updateEntity",
    id: input.id,
    set: input.set ?? [],
    unset: input.unset ?? [],
  };
}

/**
 * Creates a DeleteEntity operation.
 *
 * Transitions the entity to DELETED state. The entity can be restored
 * using {@link restoreEntity}.
 *
 * @param id - The entity ID to delete
 * @returns A DeleteEntity operation
 */
export function deleteEntity(id: Id): DeleteEntity {
  return { type: "deleteEntity", id };
}

/**
 * Creates a RestoreEntity operation.
 *
 * Transitions a deleted entity back to ALIVE state.
 *
 * @param id - The entity ID to restore
 * @returns A RestoreEntity operation
 */
export function restoreEntity(id: Id): RestoreEntity {
  return { type: "restoreEntity", id };
}

/**
 * Input for creating a CreateRelation operation.
 * Includes all fields from CreateRelation except the `type` discriminator.
 */
export type CreateRelationInput = Omit<CreateRelation, "type">;

/**
 * Creates a CreateRelation operation.
 *
 * Creates a directed relation between two entities with a specified type.
 * Relations can optionally have a position for ordering and reference
 * entities in other spaces/versions.
 *
 * @param input - The relation configuration
 * @returns A CreateRelation operation
 *
 * @example
 * ```ts
 * const op = createRelation({
 *   id: relationId,
 *   relationType: relationTypes.types(),
 *   from: entityA,
 *   to: entityB,
 * });
 * ```
 */
export function createRelation(input: CreateRelationInput): CreateRelation {
  return {
    type: "createRelation",
    ...input,
  };
}

/**
 * Input for creating an UpdateRelation operation.
 */
export interface UpdateRelationInput
  extends Omit<UpdateRelation, "type" | "unset"> {
  /** Fields to unset on the relation. */
  unset?: UnsetRelationField[];
}

/**
 * Creates an UpdateRelation operation.
 *
 * Updates an existing relation's position or cross-space references.
 * Can also unset optional fields like position or space/version references.
 *
 * @param input - The update configuration
 * @returns An UpdateRelation operation
 *
 * @example
 * ```ts
 * const op = updateRelation({
 *   id: relationId,
 *   position: "abc",
 * });
 * ```
 */
export function updateRelation(input: UpdateRelationInput): UpdateRelation {
  const { unset, ...rest } = input;
  return {
    type: "updateRelation",
    unset: unset ?? [],
    ...rest,
  };
}

/**
 * Creates a DeleteRelation operation.
 *
 * Transitions the relation to DELETED state. The relation can be restored
 * using {@link restoreRelation}.
 *
 * @param id - The relation ID to delete
 * @returns A DeleteRelation operation
 */
export function deleteRelation(id: Id): DeleteRelation {
  return { type: "deleteRelation", id };
}

/**
 * Creates a RestoreRelation operation.
 *
 * Transitions a deleted relation back to ALIVE state.
 *
 * @param id - The relation ID to restore
 * @returns A RestoreRelation operation
 */
export function restoreRelation(id: Id): RestoreRelation {
  return { type: "restoreRelation", id };
}

/**
 * Input for creating a CreateValueRef operation.
 * Includes all fields from CreateValueRef except the `type` discriminator.
 */
export type CreateValueRefInput = Omit<CreateValueRef, "type">;

/**
 * Creates a CreateValueRef operation.
 *
 * Creates a reference to a value on another entity, optionally in another space.
 * This allows sharing values across entities without duplication.
 *
 * @param input - The value reference configuration
 * @returns A CreateValueRef operation
 *
 * @example
 * ```ts
 * const op = createValueRef({
 *   id: refId,
 *   entity: sourceEntityId,
 *   property: properties.name(),
 * });
 * ```
 */
export function createValueRef(input: CreateValueRefInput): CreateValueRef {
  return {
    type: "createValueRef",
    ...input,
  };
}
