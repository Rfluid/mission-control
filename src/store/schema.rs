// @generated shape; keep in sync with migrations/ (`diesel print-schema`).

diesel::table! {
    missions (id) {
        id -> Text,
        title -> Text,
        session -> Nullable<Text>,
        project_dir -> Nullable<Text>,
        token -> Text,
        created -> BigInt,
        archived -> Bool,
    }
}

diesel::table! {
    records (mission, collection, key) {
        mission -> Text,
        collection -> Text,
        key -> Text,
        state -> Nullable<Text>,
        body -> Text,
        updated -> BigInt,
    }
}

diesel::table! {
    events (seq) {
        seq -> BigInt,
        mission -> Text,
        kind -> Text,
        text -> Text,
        data -> Nullable<Text>,
        at -> BigInt,
    }
}

diesel::table! {
    mission_config (mission, version) {
        mission -> Text,
        version -> BigInt,
        doc -> Text,
        note -> Nullable<Text>,
        at -> BigInt,
    }
}

diesel::table! {
    cursors (mission, session) {
        mission -> Text,
        session -> Text,
        seq -> BigInt,
        updated -> BigInt,
    }
}

diesel::joinable!(records -> missions (mission));
diesel::joinable!(cursors -> missions (mission));
diesel::joinable!(events -> missions (mission));
diesel::joinable!(mission_config -> missions (mission));
diesel::allow_tables_to_appear_in_same_query!(missions, records, events, mission_config, cursors);
