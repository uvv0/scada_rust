//! Кэш шаблонов скриптов и разрешенных binding-планов для ускорения выполнения script-mode.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::script::Script;
use crate::types::GScriptRow;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Ключ шаблона скрипта: группа + версия.
pub struct TemplateKey {
    pub grup: i32,
    pub ver: i32,
}

#[derive(Clone, Debug)]
/// Связка логического индекса скрипта с физическим регистром и адресом.
pub struct RegBinding {
    pub logical: i32,
    pub reg_id: i32,
    pub addr: i32,
}

#[derive(Clone, Debug)]
/// Кэшированный шаблон группы: исходная строка `g_script` и распарсенные PRE/POST.
pub struct TemplateBundle {
    #[allow(dead_code)]
    pub key: TemplateKey,
    pub row: GScriptRow,
    pub pre: Option<Arc<Script>>,
    pub post: Option<Arc<Script>>,
    pub used_keys: Arc<Vec<i32>>,
}

#[derive(Clone, Debug)]
/// Разрешённый план выполнения: шаблон + карта binding по logical-индексу.
pub struct ResolvedPlan {
    pub template: TemplateBundle,
    pub binding_by_logical: Arc<HashMap<i32, RegBinding>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PlanKey {
    kpz_id: i32,
    template: TemplateKey,
    bindings_sig: u64,
}

#[derive(Clone)]
/// Память-ограниченный кэш шаблонов скриптов.
pub struct ScriptCache {
    template_cache: HashMap<TemplateKey, Option<TemplateBundle>>,
    plan_cache: HashMap<PlanKey, Arc<ResolvedPlan>>,
    dirty: bool,
}

impl ScriptCache {
    /// Создаёт пустой кэш шаблонов.
    ///
    /// # Returns
    /// - Новый экземпляр `ScriptCache`.
    pub fn new() -> Self {
        Self {
            template_cache: HashMap::new(),
            plan_cache: HashMap::new(),
            dirty: false,
        }
    }

    #[allow(dead_code)]
    /// Полностью очищает кэш шаблонов.
    pub fn clear(&mut self) {
        if !self.template_cache.is_empty() || !self.plan_cache.is_empty() {
            self.dirty = true;
        }
        self.template_cache.clear();
        self.plan_cache.clear();
    }

    /// Удаляет все шаблоны заданной группы из кэша.
    ///
    /// # Parameters
    /// - `grup`: идентификатор группы.
    pub fn invalidate_group(&mut self, grup: i32) {
        let t_before = self.template_cache.len();
        let p_before = self.plan_cache.len();
        self.template_cache.retain(|k, _| k.grup != grup);
        self.plan_cache.retain(|k, _| k.template.grup != grup);
        if self.template_cache.len() != t_before || self.plan_cache.len() != p_before {
            self.dirty = true;
        }
    }

    /// Инвалидация plan-cache для конкретного КПЗ.
    ///
    /// # Parameters
    /// - `kpz_id`: идентификатор КПЗ.
    pub fn invalidate_kpz(&mut self, kpz_id: i32) {
        let before = self.plan_cache.len();
        self.plan_cache.retain(|k, _| k.kpz_id != kpz_id);
        if self.plan_cache.len() != before {
            self.dirty = true;
        }
    }

    /// Сливает шаблоны из другого кэша в текущий.
    ///
    /// # Parameters
    /// - `other`: источник с дополнительными/обновлёнными шаблонами.
    pub fn merge_from(&mut self, other: &ScriptCache) {
        if other.template_cache.is_empty() && other.plan_cache.is_empty() {
            return;
        }
        for (k, v) in &other.template_cache {
            self.template_cache.insert(*k, v.clone());
        }
        for (k, v) in &other.plan_cache {
            self.plan_cache.insert(*k, Arc::clone(v));
        }
        self.dirty = true;
    }

    /// Returns a memory-light cache snapshot for a worker:
    /// only templates for enabled groups and plans for the same kpz+groups.
    pub fn clone_for_worker(&self, kpz_id: i32, enabled_groups: &HashSet<i32>) -> Self {
        let mut template_cache = HashMap::new();
        for (k, v) in &self.template_cache {
            if enabled_groups.contains(&k.grup) {
                template_cache.insert(*k, v.clone());
            }
        }
        let mut plan_cache = HashMap::new();
        for (k, v) in &self.plan_cache {
            if k.kpz_id == kpz_id && enabled_groups.contains(&k.template.grup) {
                plan_cache.insert(*k, Arc::clone(v));
            }
        }

        Self {
            template_cache,
            plan_cache,
            dirty: false,
        }
    }

    /// Показывает, менялся ли кэш с момента последней синхронизации.
    ///
    /// # Returns
    /// - `true`, если были изменения.
    /// - `false`, если кэш без новых модификаций.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Возвращает (или создаёт) кэшированный шаблон для строки `g_script`.
    ///
    /// # Parameters
    /// - `row`: строка скриптовой конфигурации группы.
    ///
    /// # Returns
    /// - `Some(TemplateBundle)`: шаблон доступен и включён.
    /// - `None`: скрипт отключён или не удалось собрать шаблон.
    pub fn get_template(&mut self, row: &GScriptRow) -> Option<TemplateBundle> {
        let key = TemplateKey {
            grup: row.grup,
            ver: row.ver.unwrap_or(0),
        };
        if let Some(v) = self.template_cache.get(&key) {
            return v.clone();
        }
        let bundle = build_template_bundle(row);
        self.template_cache.insert(key, bundle.clone());
        self.dirty = true;
        bundle
    }

    /// Формирует исполняемый план: шаблон + map привязок logical->register.
    ///
    /// # Parameters
    /// - `kpz_id`: идентификатор КПЗ.
    /// - `row`: строка `g_script` с текстами PRE/POST.
    /// - `bindings`: список разрешённых привязок регистров.
    ///
    /// # Returns
    /// - `Some(Arc<ResolvedPlan>)`: готовый план для выполнения.
    /// - `None`: шаблон недоступен/отключён.
    pub fn get_plan(
        &mut self,
        kpz_id: i32,
        row: &GScriptRow,
        bindings: &[RegBinding],
    ) -> Option<Arc<ResolvedPlan>> {
        let template = self.get_template(row)?;
        let key = PlanKey {
            kpz_id,
            template: template.key,
            bindings_sig: bindings_signature(bindings),
        };
        if let Some(plan) = self.plan_cache.get(&key) {
            return Some(Arc::clone(plan));
        }
        let mut m = HashMap::with_capacity(bindings.len());
        for b in bindings {
            m.insert(b.logical, b.clone());
        }
        let plan = Arc::new(ResolvedPlan {
            template,
            binding_by_logical: Arc::new(m),
        });
        self.plan_cache.insert(key, Arc::clone(&plan));
        self.dirty = true;
        Some(plan)
    }
}

fn bindings_signature(bindings: &[RegBinding]) -> u64 {
    let mut entries: Vec<(i32, i32, i32)> = bindings
        .iter()
        .map(|b| (b.logical, b.reg_id, b.addr))
        .collect();
    entries.sort_unstable();

    let mut hasher = DefaultHasher::new();
    entries.hash(&mut hasher);
    hasher.finish()
}

fn build_template_bundle(row: &GScriptRow) -> Option<TemplateBundle> {
    if row.en == Some(false) {
        return None;
    }

    let pre_src = row.pre_src.as_deref().unwrap_or("").trim();
    let post_src = row.post_src.as_deref().unwrap_or("").trim();

    let pre = if pre_src.len() >= 3 {
        match Script::parse(pre_src) {
            Ok(s) => Some(Arc::new(s)),
            Err(e) => {
                tracing::warn!(
                    grup = row.grup,
                    ver = row.ver.unwrap_or(0),
                    err = %e,
                    "script PRE parse failed"
                );
                None
            }
        }
    } else {
        None
    };

    let post = if post_src.len() >= 3 {
        match Script::parse(post_src) {
            Ok(s) => Some(Arc::new(s)),
            Err(e) => {
                tracing::warn!(
                    grup = row.grup,
                    ver = row.ver.unwrap_or(0),
                    err = %e,
                    "script POST parse failed"
                );
                None
            }
        }
    } else {
        None
    };

    let mut used_keys = Vec::new();
    if let Some(s) = pre.as_ref() {
        used_keys.extend_from_slice(s.used_rv_keys());
    }
    if let Some(s) = post.as_ref() {
        used_keys.extend_from_slice(s.used_rv_keys());
    }
    used_keys.sort_unstable();
    used_keys.dedup();

    Some(TemplateBundle {
        key: TemplateKey {
            grup: row.grup,
            ver: row.ver.unwrap_or(0),
        },
        row: row.clone(),
        pre,
        post,
        used_keys: Arc::new(used_keys),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_row(grup: i32, ver: i32) -> GScriptRow {
        GScriptRow {
            grup,
            pre_src: Some("let a = rv(10); reg(100)=a;".to_string()),
            post_src: Some("let b = rv(20); reg(200)=b;".to_string()),
            max_k: Some(2),
            max_words: Some(125),
            en: Some(true),
            ver: Some(ver),
        }
    }

    fn mk_bindings(order: &[i32]) -> Vec<RegBinding> {
        order
            .iter()
            .map(|logical| RegBinding {
                logical: *logical,
                reg_id: 1000 + *logical,
                addr: 30000 + *logical,
            })
            .collect()
    }

    #[test]
    fn template_cache_reuses_same_group_and_version() {
        let mut c = ScriptCache::new();
        let row = mk_row(7, 1);

        let t1 = c.get_template(&row).expect("template");
        let t2 = c.get_template(&row).expect("template");

        assert_eq!(t1.key, t2.key);
        assert_eq!(t1.used_keys.as_ref(), &[10, 20]);
    }

    #[test]
    fn get_plan_builds_binding_map() {
        let mut c = ScriptCache::new();
        let row = mk_row(9, 2);

        let b1 = mk_bindings(&[1, 2, 3]);
        let b2 = mk_bindings(&[3, 1, 2]);

        let p1 = c.get_plan(11, &row, &b1).expect("plan1");
        let p2 = c.get_plan(11, &row, &b2).expect("plan2");

        assert_eq!(p1.binding_by_logical.len(), 3);
        assert_eq!(p2.binding_by_logical.len(), 3);
        assert_eq!(
            p1.binding_by_logical.get(&1).map(|b| b.addr),
            p2.binding_by_logical.get(&1).map(|b| b.addr)
        );
    }

    #[test]
    fn get_plan_reuses_cached_plan_for_same_bindings() {
        let mut c = ScriptCache::new();
        let row = mk_row(9, 2);

        let b1 = mk_bindings(&[1, 2, 3]);
        let b2 = mk_bindings(&[3, 1, 2]);

        let p1 = c.get_plan(11, &row, &b1).expect("plan1");
        let p2 = c.get_plan(11, &row, &b2).expect("plan2");

        assert!(Arc::ptr_eq(&p1, &p2));
    }

    #[test]
    fn invalidate_group_drops_template_and_plan_cache() {
        let mut c = ScriptCache::new();
        let row = mk_row(5, 1);
        let bindings = mk_bindings(&[1, 2]);

        let p1 = c.get_plan(1, &row, &bindings).expect("plan");
        c.invalidate_group(5);
        let p2 = c.get_plan(1, &row, &bindings).expect("plan");

        assert_eq!(p1.template.key, p2.template.key);
        assert!(!Arc::ptr_eq(&p1, &p2));
    }

    #[test]
    fn invalidate_kpz_drops_only_target_kpz_plan_cache() {
        let mut c = ScriptCache::new();
        let row = mk_row(6, 1);
        let bindings = mk_bindings(&[1]);

        let p1_k1 = c.get_plan(1, &row, &bindings).expect("k1 plan");
        let p1_k2 = c.get_plan(2, &row, &bindings).expect("k2 plan");
        c.invalidate_kpz(1);
        let p2_k1 = c.get_plan(1, &row, &bindings).expect("k1 plan");
        let p2_k2 = c.get_plan(2, &row, &bindings).expect("k2 plan");

        assert_eq!(p1_k1.template.key, p2_k1.template.key);
        assert!(!Arc::ptr_eq(&p1_k1, &p2_k1));
        assert!(Arc::ptr_eq(&p1_k2, &p2_k2));
    }
}
