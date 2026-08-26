use crate::app::Ss7App;
use crate::models::WebAccountRow;

impl Ss7App {
    pub(crate) fn sync_web_account_from_selected(&mut self) {
        if let Some(row) = self
            .web_account_selected_id
            .and_then(|id| self.web_accounts.iter().find(|x| x.id == id))
        {
            self.web_account_login = row.login.clone();
            self.web_account_password.clear();
            self.web_account_role = row.role.clone();
            self.web_account_enabled = row.enabled;
            self.web_account_kpz_from = row.kpz_from.map(|v| v.to_string()).unwrap_or_default();
            self.web_account_kpz_to = row.kpz_to.map(|v| v.to_string()).unwrap_or_default();
        } else {
            self.web_account_login.clear();
            self.web_account_password.clear();
            self.web_account_role = "viewer".to_string();
            self.web_account_enabled = true;
            self.web_account_kpz_from.clear();
            self.web_account_kpz_to.clear();
        }
        self.web_account_err = None;
        self.web_account_status = None;
    }

    pub(crate) fn reload_web_accounts(&mut self) {
        match self.db.get_web_accounts() {
            Ok(rows) => {
                self.web_accounts = rows;
                if let Some(id) = self.web_account_selected_id {
                    if !self.web_accounts.iter().any(|x| x.id == id) {
                        self.web_account_selected_id = self.web_accounts.first().map(|x| x.id);
                    }
                } else {
                    self.web_account_selected_id = self.web_accounts.first().map(|x| x.id);
                }
                self.sync_web_account_from_selected();
            }
            Err(e) => {
                self.web_account_err = Some(format!("load accounts failed: {e}"));
            }
        }
    }

    pub(crate) fn open_accounts_window(&mut self) {
        self.reload_web_accounts();
        self.accounts_window_open = true;
    }

    pub(crate) fn new_web_account_form(&mut self) {
        self.web_account_selected_id = None;
        self.web_account_login.clear();
        self.web_account_password.clear();
        self.web_account_role = "viewer".to_string();
        self.web_account_enabled = true;
        self.web_account_kpz_from.clear();
        self.web_account_kpz_to.clear();
        self.web_account_err = None;
        self.web_account_status = None;
    }

    pub(crate) fn save_web_account(&mut self) {
        let login = self.web_account_login.trim().to_string();
        if login.is_empty() {
            self.web_account_err = Some("login is required".to_string());
            return;
        }
        let password = self.web_account_password.trim().to_string();
        if password.is_empty() && self.web_account_selected_id.is_none() {
            self.web_account_err = Some("password is required".to_string());
            return;
        }
        let role = self.web_account_role.trim().to_string();
        if role.is_empty() {
            self.web_account_err = Some("role is required".to_string());
            return;
        }
        let kpz_from = if self.web_account_kpz_from.trim().is_empty() {
            None
        } else {
            match self.web_account_kpz_from.trim().parse::<i32>() {
                Ok(v) => Some(v),
                Err(_) => {
                    self.web_account_err = Some("kpz_from must be integer".to_string());
                    return;
                }
            }
        };
        let kpz_to = if self.web_account_kpz_to.trim().is_empty() {
            None
        } else {
            match self.web_account_kpz_to.trim().parse::<i32>() {
                Ok(v) => Some(v),
                Err(_) => {
                    self.web_account_err = Some("kpz_to must be integer".to_string());
                    return;
                }
            }
        };
        if let (Some(from), Some(to)) = (kpz_from, kpz_to)
            && from > to
        {
            self.web_account_err = Some("kpz_from must be <= kpz_to".to_string());
            return;
        }
        let row = WebAccountRow {
            id: self.web_account_selected_id.unwrap_or(0),
            login,
            password,
            role,
            enabled: self.web_account_enabled,
            kpz_from,
            kpz_to,
        };
        match self.db.upsert_web_account(&row) {
            Ok(id) => {
                self.web_account_selected_id = Some(id);
                self.reload_web_accounts();
                self.web_account_selected_id = Some(id);
                self.sync_web_account_from_selected();
                self.web_account_status = Some("account saved".to_string());
                self.web_account_err = None;
            }
            Err(e) => {
                self.web_account_err = Some(format!("save account failed: {e}"));
            }
        }
    }

    pub(crate) fn delete_web_account(&mut self) {
        let Some(id) = self.web_account_selected_id else {
            self.web_account_err = Some("select account first".to_string());
            return;
        };
        match self.db.delete_web_account(id) {
            Ok(()) => {
                self.reload_web_accounts();
                self.new_web_account_form();
                self.web_account_status = Some("account deleted".to_string());
                self.web_account_err = None;
            }
            Err(e) => {
                self.web_account_err = Some(format!("delete account failed: {e}"));
            }
        }
    }
}
