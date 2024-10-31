// @generated automatically by Diesel CLI.

diesel::table! {
    devices (id) {
        id -> Text,
        ip -> Nullable<Text>,
        port -> Nullable<Integer>,
        server_port -> Nullable<Integer>,
        status -> Integer,
        self_device -> Bool,
        updated_at -> Integer,
    }
}

diesel::table! {
    file_metadata (id) {
        id -> Integer,
        code -> Text,
        file_name -> Text,
        file_size -> Integer,
        file_type -> Text,
        local_path -> Text,
        created_at -> Integer,
        updated_at -> Integer,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    devices,
    file_metadata,
);
