// @generated automatically by Diesel CLI.

diesel::table! {
    buckets (id) {
        id -> Text,
        client_id -> Text,
        name -> Text,
        images_only -> Integer,
        created_at -> BigInt,
    }
}

diesel::table! {
    clients (id) {
        id -> Text,
        name -> Text,
        status -> Text,
        created_at -> BigInt,
        default_bucket_id -> Nullable<Text>,
    }
}

diesel::table! {
    dirs (id) {
        id -> Text,
        bucket_id -> Text,
        name -> Text,
        label -> Text,
        file_count -> Integer,
        created_at -> BigInt,
        updated_at -> BigInt,
    }
}

diesel::table! {
    files (id) {
        id -> Text,
        dir_id -> Text,
        name -> Text,
        filename -> Text,
        content_type -> Text,
        size -> BigInt,
        is_image -> Integer,
        img_dimension -> Nullable<Text>,
        img_versions -> Nullable<Text>,
        created_at -> BigInt,
        updated_at -> BigInt,
        img_taken_at -> Nullable<BigInt>,
    }
}

diesel::table! {
    users (id) {
        id -> Text,
        client_id -> Text,
        username -> Text,
        password -> Text,
        status -> Text,
        roles -> Text,
        created_at -> BigInt,
        updated_at -> BigInt,
    }
}

diesel::joinable!(buckets -> clients (client_id));
diesel::joinable!(dirs -> buckets (bucket_id));
diesel::joinable!(users -> clients (client_id));

diesel::allow_tables_to_appear_in_same_query!(buckets, clients, dirs, files, users,);
