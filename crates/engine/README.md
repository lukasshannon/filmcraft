# filmcraft-engine

The headless editing engine dispatches commands over a session, owns projects,
undo, persistence and background jobs, and receives host file services by trait.

Hosts with scoped document providers can implement `Services::publish_export`.
The streaming export worker calls it after a successful encode, keeps the job
pending until publication finishes, and records publication failures as errors
while retaining local output. The default filesystem host is unchanged.
`streamed_export_provider_failure_is_a_job_error_and_keeps_local_file` covers this
failure path. Asynchronous project-save provider copies belong to the Android
host; durable project and recovery serialization continue to use engine services.
