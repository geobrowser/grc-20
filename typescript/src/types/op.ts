import type { Id } from "./id.js";
import type { DataType, PropertyValue } from "./value.js";

/**
 * Specifies which language slot to clear for an UnsetProperty.
 */
export type UnsetLanguage =
  | { type: "all" }
  | { type: "nonLinguistic" }
  | { type: "specific"; language: Id };

/**
 * Fields that can be unset on a relation.
 */
export type UnsetRelationField =
  | "fromSpace"
  | "fromVersion"
  | "toSpace"
  | "toVersion"
  | "position";

/**
 * Specifies a property to unset, with optional language targeting (TEXT only).
 */
export interface UnsetProperty {
  property: Id;
  language: UnsetLanguage;
}

/**
 * Creates an entity (spec Section 3.2).
 *
 * If the entity does not exist, creates it. If it already exists,
 * this acts as an update: values are applied as set_properties (LWW).
 */
export interface CreateEntity {
  type: "createEntity";
  id: Id;
  values: PropertyValue[];
}

/**
 * Updates an existing entity (spec Section 3.2).
 *
 * Application order within op:
 * 1. unsetProperties
 * 2. setProperties
 */
export interface UpdateEntity {
  type: "updateEntity";
  id: Id;
  setProperties: PropertyValue[];
  unsetProperties: UnsetProperty[];
}

/**
 * Deletes an entity (spec Section 3.2).
 *
 * Transitions the entity to DELETED state.
 */
export interface DeleteEntity {
  type: "deleteEntity";
  id: Id;
}

/**
 * Restores a deleted entity (spec Section 3.2).
 */
export interface RestoreEntity {
  type: "restoreEntity";
  id: Id;
}

/**
 * Creates a relation (spec Section 3.3).
 */
export interface CreateRelation {
  type: "createRelation";
  id: Id;
  relationType: Id;
  from: Id;
  to: Id;
  fromSpace?: Id;
  fromVersion?: Id;
  toSpace?: Id;
  toVersion?: Id;
  entity?: Id;
  position?: string;
}

/**
 * Updates a relation's mutable fields (spec Section 3.3).
 *
 * The structural fields (entity, type, from, to) are immutable.
 * The space pins, version pins, and position can be updated or unset.
 *
 * Application order within op:
 * 1. unset
 * 2. set fields
 */
export interface UpdateRelation {
  type: "updateRelation";
  id: Id;
  fromSpace?: Id;
  fromVersion?: Id;
  toSpace?: Id;
  toVersion?: Id;
  position?: string;
  unset: UnsetRelationField[];
}

/**
 * Deletes a relation (spec Section 3.3).
 */
export interface DeleteRelation {
  type: "deleteRelation";
  id: Id;
}

/**
 * Restores a deleted relation (spec Section 3.3).
 */
export interface RestoreRelation {
  type: "restoreRelation";
  id: Id;
}

/**
 * Creates a property in the schema (spec Section 3.4).
 */
export interface CreateProperty {
  type: "createProperty";
  id: Id;
  dataType: DataType;
}

/**
 * An atomic operation that modifies graph state (spec Section 3.1).
 */
export type Op =
  | CreateEntity
  | UpdateEntity
  | DeleteEntity
  | RestoreEntity
  | CreateRelation
  | UpdateRelation
  | DeleteRelation
  | RestoreRelation
  | CreateProperty;

/**
 * Op type codes for wire encoding.
 */
export const OP_TYPE_CREATE_ENTITY = 1;
export const OP_TYPE_UPDATE_ENTITY = 2;
export const OP_TYPE_DELETE_ENTITY = 3;
export const OP_TYPE_RESTORE_ENTITY = 4;
export const OP_TYPE_CREATE_RELATION = 5;
export const OP_TYPE_UPDATE_RELATION = 6;
export const OP_TYPE_DELETE_RELATION = 7;
export const OP_TYPE_RESTORE_RELATION = 8;
export const OP_TYPE_CREATE_PROPERTY = 9;

/**
 * Returns the op type code for wire encoding.
 */
export function opTypeCode(op: Op): number {
  switch (op.type) {
    case "createEntity":
      return OP_TYPE_CREATE_ENTITY;
    case "updateEntity":
      return OP_TYPE_UPDATE_ENTITY;
    case "deleteEntity":
      return OP_TYPE_DELETE_ENTITY;
    case "restoreEntity":
      return OP_TYPE_RESTORE_ENTITY;
    case "createRelation":
      return OP_TYPE_CREATE_RELATION;
    case "updateRelation":
      return OP_TYPE_UPDATE_RELATION;
    case "deleteRelation":
      return OP_TYPE_DELETE_RELATION;
    case "restoreRelation":
      return OP_TYPE_RESTORE_RELATION;
    case "createProperty":
      return OP_TYPE_CREATE_PROPERTY;
  }
}

/**
 * Validates a position string according to spec rules.
 *
 * Position strings must:
 * - Not be empty
 * - Only contain characters 0-9, A-Z, a-z (62 chars, ASCII order)
 * - Not exceed 64 characters
 */
export function validatePosition(pos: string): string | undefined {
  if (pos.length === 0) {
    return "position cannot be empty";
  }
  if (pos.length > 64) {
    return "position exceeds 64 characters";
  }
  for (const c of pos) {
    if (!/^[0-9A-Za-z]$/.test(c)) {
      return `position contains invalid character: ${c}`;
    }
  }
  return undefined;
}
