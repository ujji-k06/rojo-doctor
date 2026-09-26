--!strict

export type Severity = "warning" | "error"

export type Diagnostic = {
	code: string,
	severity: Severity,
	subject: string,
	message: string,
	help: string?,
}

export type Report = {
	project: string,
	resolved_paths: number,
	warnings: number,
	errors: number,
	diagnostics: { Diagnostic },
}

export type MappedPath = {
	instance_path: string,
	instance_segments: { string },
	configured_path: string,
	resolved_path: string,
	optional: boolean,
	has_children: boolean,
	class_name: string?,
	child_names: { string },
}

export type ProjectNode = {
	class_name: string?,
	path: string?,
	optional_path: string?,
	children: { [string]: ProjectNode },
}

export type LoadedProject = {
	path: string,
	root_dir: string,
	name: string?,
	tree: ProjectNode,
}

return {}
