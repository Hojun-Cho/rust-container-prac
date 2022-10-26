use std::{
    fs,
    path::PathBuf,
    process::{self, Command, ExitStatus},
};

use nix::{
    mount, sched,
    sys::wait::{WaitStatus, waitpid},
    sys::{signal::Signal, stat},
    unistd, Error,
};

const CGROUP_NAME: &str = "vasquod-container";
const HOSTNAME: &str = "vasquod";
const STACK_SIZE: usize = 1024 * 1024;

const PROC: &str = "proc";
const CGROUP_PATH: &str = "/sys/fs/cgroup/pids";

const ROOT_PATH: &str = "/";
const OLD_ROOT_PATH: &str = "/.oldroot";

struct Runner<'a> {
    command: &'a str,
    command_args: &'a [&'a str],
}

impl<'a> Runner<'a> {
    fn run(&self) -> isize {
        let exit_status: ExitStatus = Command::new(self.command)
            .args(self.command_args)
            .spawn()
            .expect("Failed to run")
            .wait()
            .unwrap();

        match exit_status.code() {
            Some(code) => code as isize,
            None => -1,
        }
    }
}

pub fn run_container(rootfs: &str, command: &str, args: Vec<&str>) {
    let group_name = CGROUP_NAME;
    let hostname = HOSTNAME;
    let stack: &mut [u8; STACK_SIZE] = &mut [0; STACK_SIZE];

    let callback = Box::new(|| spawn_child(hostname, group_name, rootfs, command, args.as_slice()));

    let clone_flags = sched::CloneFlags::CLONE_NEWNS
        | sched::CloneFlags::CLONE_NEWPID
        | sched::CloneFlags::CLONE_NEWCGROUP
        | sched::CloneFlags::CLONE_NEWUTS
        | sched::CloneFlags::CLONE_NEWIPC
        | sched::CloneFlags::CLONE_NEWNET;
    let child_pid = sched::clone(callback, stack, clone_flags, Some(Signal::SIGCHLD as i32))
        .expect("Failed to create child process");
    let _= waitpid(child_pid, None).unwrap();
}

fn spawn_child<'a>(
    hostname: &str,
    cgroup_name: &str,
    rootfs: &str,
    command: &'a str,
    command_args: &'a [&'a str],
) -> isize {
    set_namespace();
    cgroup_init(cgroup_name);
    set_hostname(hostname);

    mount_root_fs(rootfs);
    set_rootfs(rootfs);
    unmount_host_root_fs();
    mount_proc();

    // The Drop impl for Runner is the equivalent of a try/finally
    // block to ensure we unmount regardless of what goes wrong
    let run: Runner<'a> = Runner {
        command,
        command_args,
    };
    run.run()
}

fn set_namespace() {
    // Unshare mount, network, IPC and UTS namespace
    sched::unshare(
        sched::CloneFlags::CLONE_NEWNS
            | sched::CloneFlags::CLONE_NEWNET
            | sched::CloneFlags::CLONE_NEWUTS
            | sched::CloneFlags::CLONE_NEWPID
            | sched::CloneFlags::CLONE_NEWUTS,
    )
    .expect("Failed to unshare");
}

fn cgroup_init(group_name: &str) {
    let mut cgroups_path = PathBuf::from(CGROUP_PATH);
    if !cgroups_path.exists() {
        eprint!("Can't Find Cgroup");
        process::exit(0);
    }

    cgroups_path.push(group_name);
    if !cgroups_path.exists() {
        // if path not exist
        fs::create_dir(&cgroups_path).unwrap();
        let mut permission = fs::metadata(&cgroups_path).unwrap().permissions();
        fs::set_permissions(&cgroups_path, permission).ok();
    }

    // add cgroup file
    let pids_max = cgroups_path.join("pids.max");
    let notify_on_release = cgroups_path.join("notify_on_release");
    let procs = cgroups_path.join("cgroup.procs");

    fs::write(pids_max, b"20").unwrap();
    fs::write(notify_on_release, b"1").unwrap();
    fs::write(procs, format!("{}", unistd::getpid())).unwrap();
}

fn mount_proc() {
    const NONE: Option<&'static [u8]> = None;
    mount::mount(Some(PROC), PROC, Some(PROC), mount::MsFlags::empty(), NONE)
        .expect("Failed to mount the /proc");
}

fn mount_root_fs(rootfs: &str) {
    mount::mount(
        Some(rootfs),
        rootfs,
        None::<&str>,
        mount::MsFlags::MS_BIND | mount::MsFlags::MS_REC,
        None::<&str>,
    )
    .unwrap();
}

fn set_rootfs(rootfs: &str) {
    let p_root_fs = PathBuf::from(rootfs).join(OLD_ROOT_PATH);
    let _rm_status = fs::remove_dir_all(&p_root_fs).map_err(|_| Error::InvalidPath);
    let _mkdir_status = unistd::mkdir(
        &p_root_fs,
        stat::Mode::S_IRWXU | stat::Mode::S_IRWXG | stat::Mode::S_IRWXO,
    );
    let _pivot_root_status = unistd::pivot_root(rootfs, &p_root_fs);
    let _chdir_status = unistd::chdir(ROOT_PATH);
}

fn unmount_host_root_fs() {
    let _status = mount::umount2(OLD_ROOT_PATH, mount::MntFlags::MNT_DETACH);
}
fn set_hostname(hostname: &str) {
    // can also use libc here
    unistd::sethostname(hostname).unwrap()
}
