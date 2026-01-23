//! Simple decoder to inspect GRC-20 files.

use std::fs;
use grc_20::{decode_edit, Op, Value, CreateEntity, UpdateEntity, CreateRelation, DeleteEntity};

fn format_id(id: &[u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        id[0], id[1], id[2], id[3], id[4], id[5], id[6], id[7],
        id[8], id[9], id[10], id[11], id[12], id[13], id[14], id[15]
    )
}

fn format_value(v: &Value) -> String {
    match v {
        Value::Text { value, .. } => {
            let preview: String = value.chars().take(80).collect();
            if value.len() > 80 {
                format!("\"{}...\"", preview)
            } else {
                format!("\"{}\"", preview)
            }
        }
        Value::Int64 { value, .. } => format!("{}", value),
        Value::Float64 { value, .. } => format!("{:.6}", value),
        Value::Bool(b) => format!("{}", b),
        Value::Date(s) => format!("DATE({})", s),
        Value::Time(s) => format!("TIME({})", s),
        Value::Datetime(s) => format!("DATETIME({})", s),
        Value::Schedule(s) => format!("SCHEDULE({})", s),
        Value::Point { lat, lon, alt } => {
            if let Some(a) = alt {
                format!("POINT({}, {}, {})", lat, lon, a)
            } else {
                format!("POINT({}, {})", lat, lon)
            }
        }
        Value::Rect { min_lat, min_lon, max_lat, max_lon } => {
            format!("RECT({}, {}, {}, {})", min_lat, min_lon, max_lat, max_lon)
        }
        Value::Bytes(b) => format!("BYTES[{}]", b.len()),
        Value::Decimal { exponent, mantissa, .. } => format!("DECIMAL(e={}, m={:?})", exponent, mantissa),
        Value::Embedding { sub_type, dims, .. } => format!("EMBEDDING({:?}, dims={})", sub_type, dims),
    }
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "../../data/podcast_data.grc20z".to_string());

    println!("Reading: {}", path);

    let data = fs::read(&path).expect("Failed to read file");
    println!("File size: {} bytes", data.len());

    let edit = decode_edit(&data).expect("Failed to decode");

    println!("\n=== Edit Info ===");
    println!("ID: {}", format_id(&edit.id));
    if !edit.name.is_empty() {
        println!("Name: {}", edit.name);
    }
    println!("Authors: {}", edit.authors.len());
    for author in &edit.authors {
        println!("  - {}", format_id(author));
    }
    if edit.created_at != 0 {
        println!("Created at: {} (µs since epoch)", edit.created_at);
    }

    println!("\n=== Operations ({}) ===", edit.ops.len());

    let mut create_entity_count = 0;
    let mut update_entity_count = 0;
    let mut delete_entity_count = 0;
    let mut create_relation_count = 0;
    let mut update_relation_count = 0;
    let mut delete_relation_count = 0;
    let mut restore_entity_count = 0;
    let mut restore_relation_count = 0;

    let mut create_value_ref_count = 0;
    for op in &edit.ops {
        match op {
            Op::CreateEntity(_) => create_entity_count += 1,
            Op::UpdateEntity(_) => update_entity_count += 1,
            Op::DeleteEntity(_) => delete_entity_count += 1,
            Op::RestoreEntity(_) => restore_entity_count += 1,
            Op::CreateRelation(_) => create_relation_count += 1,
            Op::UpdateRelation(_) => update_relation_count += 1,
            Op::DeleteRelation(_) => delete_relation_count += 1,
            Op::RestoreRelation(_) => restore_relation_count += 1,
            Op::CreateValueRef(_) => create_value_ref_count += 1,
        }
    }
    println!("  CreateEntity: {}", create_entity_count);
    println!("  UpdateEntity: {}", update_entity_count);
    println!("  DeleteEntity: {}", delete_entity_count);
    println!("  RestoreEntity: {}", restore_entity_count);
    println!("  CreateRelation: {}", create_relation_count);
    println!("  UpdateRelation: {}", update_relation_count);
    println!("  DeleteRelation: {}", delete_relation_count);
    println!("  RestoreRelation: {}", restore_relation_count);
    println!("  CreateValueRef: {}", create_value_ref_count);

    // Show first few operations in detail
    println!("\n=== First 20 Operations (detail) ===");
    for (i, op) in edit.ops.iter().take(20).enumerate() {
        match op {
            Op::CreateEntity(CreateEntity { id, values, .. }) => {
                println!("[{}] CreateEntity {}", i, format_id(id));
                for pv in values.iter().take(5) {
                    println!("      {} = {}", format_id(&pv.property), format_value(&pv.value));
                }
                if values.len() > 5 {
                    println!("      ... and {} more values", values.len() - 5);
                }
            }
            Op::UpdateEntity(UpdateEntity { id, set_properties, unset_values, .. }) => {
                println!("[{}] UpdateEntity {}", i, format_id(id));
                for pv in set_properties.iter().take(3) {
                    println!("      SET {} = {}", format_id(&pv.property), format_value(&pv.value));
                }
                if set_properties.len() > 3 {
                    println!("      ... and {} more set values", set_properties.len() - 3);
                }
                if !unset_values.is_empty() {
                    println!("      UNSET {} values", unset_values.len());
                }
            }
            Op::CreateRelation(CreateRelation { id, relation_type, from, to, .. }) => {
                println!("[{}] CreateRelation {} ({} -> {})",
                    i, format_id(id), format_id(from), format_id(to));
                println!("      type: {}", format_id(relation_type));
            }
            Op::DeleteEntity(DeleteEntity { id, .. }) => {
                println!("[{}] DeleteEntity {}", i, format_id(id));
            }
            _ => {
                println!("[{}] {:?}", i, std::mem::discriminant(op));
            }
        }
    }
}
