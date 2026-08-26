use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::ops::Index;
use std::time::Instant;

use crate::types::KpzRow;

use super::*;

#[derive(Clone)]
pub(super) struct KpzTask {
    pub(super) kpz: KpzRow,
    pub(super) group_id: i32,
    pub(super) generation: u64,
    pub(super) next_a: Instant,
    pub(super) next_script: Instant,
    pub(super) busy_a: bool,
    pub(super) busy_s: bool,
}

#[derive(Clone, Copy)]
pub(super) enum JobKind {
    A,
    S,
}

#[derive(Clone, Copy)]
pub(super) struct Job {
    pub(super) kpz_id: i32,
    pub(super) kind: JobKind,
}

pub(super) struct JobQueue {
    next_seq: u64,
    len: usize,
    jobs_by_seq: BTreeMap<u64, Job>,
    seqs_by_kpz: HashMap<i32, VecDeque<u64>>,
    ready_kpz: BTreeSet<(u64, i32)>,
}

impl JobQueue {
    pub(super) fn new() -> Self {
        Self {
            next_seq: 0,
            len: 0,
            jobs_by_seq: BTreeMap::new(),
            seqs_by_kpz: HashMap::new(),
            ready_kpz: BTreeSet::new(),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub(super) fn len(&self) -> usize {
        self.len
    }

    pub(super) fn push_back(&mut self, job: Job) {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.push_with_seq(seq, job);
    }

    pub(super) fn pop_next_spawnable(&mut self, running_kpz: &HashSet<i32>) -> Option<Job> {
        let (head_seq, kpz_id) = self
            .ready_kpz
            .iter()
            .copied()
            .find(|(_, kpz_id)| !running_kpz.contains(kpz_id))?;
        self.ready_kpz.remove(&(head_seq, kpz_id));

        let seqs = self.seqs_by_kpz.get_mut(&kpz_id)?;
        let seq = seqs.pop_front()?;
        debug_assert_eq!(seq, head_seq);
        let job = self.jobs_by_seq.remove(&seq)?;
        self.len = self.len.saturating_sub(1);

        if let Some(next_seq) = seqs.front().copied() {
            self.ready_kpz.insert((next_seq, kpz_id));
        } else {
            self.seqs_by_kpz.remove(&kpz_id);
        }

        Some(job)
    }

    pub(super) fn retain<F>(&mut self, mut keep: F)
    where
        F: FnMut(&Job) -> bool,
    {
        let mut retained = Vec::with_capacity(self.len);
        for (seq, job) in &self.jobs_by_seq {
            if keep(job) {
                retained.push((*seq, *job));
            }
        }

        self.jobs_by_seq.clear();
        self.seqs_by_kpz.clear();
        self.ready_kpz.clear();
        self.len = 0;

        for (seq, job) in retained {
            self.push_with_seq(seq, job);
        }
    }

    fn push_with_seq(&mut self, seq: u64, job: Job) {
        self.jobs_by_seq.insert(seq, job);
        let seqs = self.seqs_by_kpz.entry(job.kpz_id).or_default();
        let was_empty = seqs.is_empty();
        seqs.push_back(seq);
        if was_empty {
            self.ready_kpz.insert((seq, job.kpz_id));
        }
        self.len += 1;
    }
}

impl Default for JobQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl Index<usize> for JobQueue {
    type Output = Job;

    fn index(&self, index: usize) -> &Self::Output {
        self.jobs_by_seq
            .values()
            .nth(index)
            .expect("job queue index out of bounds")
    }
}

impl SchedulerState {
    /// Планирует due A/script задания по периодам, учитывает backpressure и не допускает дублирование активных задач КПЗ.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// # Возвращает
    /// - `()`: помещает due-задания в очередь с учетом лимитов.
    /// # Пример
    /// - `state.dispatch_due_work();`
    pub(super) fn dispatch_due_work(&mut self) {
        let now = Instant::now();
        for t in self.tasks.values_mut() {
            if self.queue.len() >= self.max_queue {
                self.dropped_backpressure += 1;
                break;
            }
            if t.kpz.start != 1 {
                continue;
            }
            let period_a = Duration::from_secs(t.kpz.t_a.max(1) as u64);
            let period_s = Duration::from_secs(t.kpz.t_script.max(1) as u64);

            if !t.busy_a && now >= t.next_a {
                let mut due_a: usize = 0;
                while t.next_a <= now {
                    t.next_a += period_a;
                    due_a += 1;
                }
                if due_a > 0 && self.queue.len() < self.max_queue {
                    t.busy_a = true;
                    self.queue.push_back(Job {
                        kpz_id: t.kpz.id,
                        kind: JobKind::A,
                    });
                    if due_a > 1 {
                        self.dropped_backpressure += (due_a - 1) as u64;
                    }
                } else if due_a > 0 {
                    self.dropped_backpressure += due_a as u64;
                }
            }
            if !t.busy_s && now >= t.next_script {
                let mut due_s: usize = 0;
                while t.next_script <= now {
                    t.next_script += period_s;
                    due_s += 1;
                }
                if due_s > 0 && self.queue.len() < self.max_queue {
                    t.busy_s = true;
                    self.queue.push_back(Job {
                        kpz_id: t.kpz.id,
                        kind: JobKind::S,
                    });
                    if due_s > 1 {
                        self.dropped_backpressure += (due_s - 1) as u64;
                    }
                } else if due_s > 0 {
                    self.dropped_backpressure += due_s as u64;
                }
            }
        }
    }
}
