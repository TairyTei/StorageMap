use crate::{
    dialog::run_dialog,
    ui::{PgConnectDialog, PgConnectionData},
};

pub async fn pg_connect_prompt() -> Option<PgConnectionData> {
    let dlg = PgConnectDialog::new().ok()?;
    dlg.set_pg_connection_data(PgConnectionData {
        database_value: "warehouse".into(),
        host_value: "localhost".into(),
        password_value: "password".into(),
        user_value: "postgres".into(),
    });

    run_dialog(dlg, |dlg, reply| {
        dlg.on_accepted({
            let reply = reply.clone();
            move |s| reply.send(Some(s))
        });
        dlg.on_cancelled(move || reply.send(None));
    })
    .await
}
