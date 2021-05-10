use crate::errors::*;
use nix::unistd::Pid;

pub fn setup() -> Result<()> {
    if let Err(err) = ionice() {
        warn!("{}", err);
    }
    if let Err(err) = nice() {
        warn!("{}", err);
    }
    Ok(())
}

pub fn nice() -> Result<()> {
    debug!("Calling nice(2) for idle priority");
    let err = unsafe { libc::nice(19) };
    if err == -1 {
        bail!("Failed to set process priority");
    }
    Ok(())
}

pub fn ionice() -> Result<()> {
    let target = ioprio::Target::ProcessGroup(Pid::from_raw(0));
    let priority = ioprio::Priority::new(ioprio::Class::Idle);
    debug!("Calling ioprio_set for idle priority");
    ioprio::set_priority(target, priority).context("Failed to ionice process group")?;
    Ok(())
}
