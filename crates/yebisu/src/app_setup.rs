// Keep trust confirmation bound to the scanned target.
use crate::{
    app::Smoke,
    setup::{Job, Reply},
};
use gpui::Context;

impl Smoke {
    pub(crate) fn start_job(&mut self, job: Job, cx: &mut Context<Self>) {
        if self.setup.busy {
            return;
        }
        self.setup.busy = true;
        self.setup.loading = matches!(job, Job::Load);
        self.setup.pending = None;
        self.setup.status = "Working…".into();
        let epoch = self.setup.epoch;
        let work = cx
            .background_executor()
            .spawn(async move { crate::setup::execute(job) });
        cx.spawn(async move |weak, cx| {
            let result = work.await;
            let _ = weak.update(cx, |this, cx| {
                // Cancel bumps the epoch: a cancelled job's reply is dropped.
                if this.setup.epoch != epoch {
                    return;
                }
                this.apply_setup_result(result);
                this.open_pending_herdr(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn apply_setup_result(&mut self, result: crate::Result<Reply>) {
        self.shell.hosts_sheet.observe(&result);
        if let Ok(Reply::Loaded(target, _)) = &result {
            self.setup_input.set_text(target.clone());
        }
        if let Ok(Reply::Hosts(hosts, _)) = &result {
            self.setup_input.set_text(hosts.selected.clone());
        }
        self.setup.apply(result);
    }
}
