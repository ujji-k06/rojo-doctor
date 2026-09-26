--!strict
local fs = require("@lune/fs")
local Types = require("./types")
local Project = require("./project")

type Diagnostic = Types.Diagnostic
type LoadedProject = Types.LoadedProject
type Report = Types.Report
type MappedPath = Types.MappedPath
type ProjectNode = Types.ProjectNode

local Checks = {}

local INIT_SCRIPT_NAMES = {
	"init.luau",
	"init.lua",
	"init.server.luau",
	"init.server.lua",
	"init.client.luau",
	"init.client.lua",
	"init.plugin.luau",
	"init.plugin.lua",
	"init.csv",
}

local DATA_MODEL_SERVICES = {
	Workspace = true,
	Players = true,
	Lighting = true,
	ReplicatedFirst = true,
	ReplicatedStorage = true,
	ServerScriptService = true,
	ServerStorage = true,
	StarterGui = true,
	StarterPack = true,
	StarterPlayer = true,
	SoundService = true,
	Chat = true,
	TextChatService = true,
	LocalizationService = true,
	TestService = true,
	HttpService = true,
	Teams = true,
	MaterialService = true,
	VoiceChatService = true,
	ProximityPromptService = true,
	SocialService = true,
	TeleportService = true,
	MarketplaceService = true,
	InsertService = true,
	Debris = true,
	TweenService = true,
	UserInputService = true,
	RunService = true,
	ContextActionService = true,
	CollectionService = true,
	PhysicsService = true,
	PathfindingService = true,
}

local function relativeSubject(project: LoadedProject, fullPath: string): string
	local normFull = Project.normalizePath(fullPath)
	local normRoot = Project.normalizePath(project.root_dir)
	if normRoot == "." or normRoot == "" then
		return normFull
	end
	if normFull:sub(1, #normRoot + 1) == normRoot .. "/" then
		return normFull:sub(#normRoot + 2)
	end
	return normFull
end

local function hasNestedProject(dir: string): boolean
	return fs.isFile(Project.joinPaths(dir, "default.project.json"))
		or fs.isFile(Project.joinPaths(dir, "default.project.jsonc"))
end

local function dirSnapshotIsFolder(dir: string): boolean
	if not fs.isDir(dir) then
		return false
	end
	for _, initName in ipairs(INIT_SCRIPT_NAMES) do
		if fs.isFile(Project.joinPaths(dir, initName)) then
			return false
		end
	end
	if hasNestedProject(dir) then
		return false
	end
	return true
end

-- 1. missing-path
local function checkMissingPaths(project: LoadedProject, mappings: { MappedPath }): ({ Diagnostic }, number)
	local diagnostics: { Diagnostic } = {}
	local resolvedCount = 0

	local hasPesde = fs.isFile(Project.joinPaths(project.root_dir, "pesde.toml"))
	local hasWally = fs.isFile(Project.joinPaths(project.root_dir, "wally.toml"))

	for _, mapping in ipairs(mappings) do
		local exists = fs.isFile(mapping.resolved_path) or fs.isDir(mapping.resolved_path)
		if exists then
			resolvedCount += 1
		elseif not mapping.optional then
			local configured = mapping.configured_path
			local isPesdePkg = configured:find("pesde_packages") ~= nil
				or configured:sub(-8) == "Packages"
				or configured:sub(-11) == "DevPackages"

			local help = "create the mapped path or update the `$path` entry"
			if isPesdePkg and hasPesde then
				help = "run `pesde install` to create this path"
			elseif (configured:sub(-8) == "Packages" or configured:sub(-11) == "DevPackages") and hasWally then
				help = "run `wally install` to create this path"
			end

			table.insert(diagnostics, {
				code = "missing-path",
				severity = "warning",
				subject = configured,
				message = string.format(
					"referenced by %s but does not exist, so Rojo cannot resolve this mapping",
					mapping.instance_path
				),
				help = help,
			})
		end
	end

	return diagnostics, resolvedCount
end

-- 2. file-with-children
local function checkFileWithChildren(_project: LoadedProject, mappings: { MappedPath }): { Diagnostic }
	local diagnostics: { Diagnostic } = {}
	for _, mapping in ipairs(mappings) do
		if fs.isFile(mapping.resolved_path) and mapping.has_children then
			table.insert(diagnostics, {
				code = "file-with-children",
				severity = "error",
				subject = mapping.configured_path,
				message = string.format(
					"%s maps to a file but also lists child instances, which Rojo cannot merge",
					mapping.instance_path
				),
				help = "remove the child instances or point `$path` at a directory",
			})
		end
	end
	return diagnostics
end

-- 3. missing-name
local function checkMissingName(project: LoadedProject): { Diagnostic }
	local diagnostics: { Diagnostic } = {}
	if project.name == nil or project.name:match("^%s*$") then
		local fileName = project.path:match("[^/]+$") or "project"
		table.insert(diagnostics, {
			code = "missing-name",
			severity = "warning",
			subject = fileName,
			message = "project has no name, so Rojo will fall back to the file name",
			help = "set the top-level `name` field",
		})
	end
	return diagnostics
end

-- 4. missing-class
local function checkMissingClass(project: LoadedProject): { Diagnostic }
	local diagnostics: { Diagnostic } = {}

	local function walkNode(node: ProjectNode, instanceSegments: { string }, parentClass: string?)
		local nodeClass = node.class_name
		local hasPath = node.path ~= nil or node.optional_path ~= nil
		local currentName = instanceSegments[#instanceSegments]

		local isDataModelRoot = #instanceSegments == 0
		local inferred: string? = nil

		if
			(parentClass == "DataModel" and currentName and DATA_MODEL_SERVICES[currentName])
			or (
				parentClass == "StarterPlayer"
				and (currentName == "StarterPlayerScripts" or currentName == "StarterCharacterScripts")
			)
		then
			inferred = currentName
		elseif parentClass == "Workspace" and currentName == "Terrain" then
			inferred = "Terrain"
		end

		local effectiveClass = nodeClass or inferred

		if not isDataModelRoot and not hasPath and effectiveClass == nil then
			local instancePathStr = table.concat(instanceSegments, ".")
			table.insert(diagnostics, {
				code = "missing-class",
				severity = "error",
				subject = instancePathStr,
				message = "has neither $className nor $path, so Rojo cannot snapshot this instance",
				help = "set `$className` or `$path`",
			})
		end

		local nextParentClass = isDataModelRoot and "DataModel" or effectiveClass

		local sortedChildren = {}
		for childName in pairs(node.children) do
			table.insert(sortedChildren, childName)
		end
		table.sort(sortedChildren)

		for _, childName in ipairs(sortedChildren) do
			table.insert(instanceSegments, childName)
			walkNode(node.children[childName], instanceSegments, nextParentClass)
			table.remove(instanceSegments)
		end
	end

	walkNode(project.tree, {}, nil)
	return diagnostics
end

-- 5. class-on-non-folder
local function checkClassOnNonFolder(_project: LoadedProject, mappings: { MappedPath }): { Diagnostic }
	local diagnostics: { Diagnostic } = {}
	for _, mapping in ipairs(mappings) do
		if mapping.class_name ~= nil then
			local isFile = fs.isFile(mapping.resolved_path)
			local isFolder = fs.isDir(mapping.resolved_path) and dirSnapshotIsFolder(mapping.resolved_path)
			if isFile or (fs.isDir(mapping.resolved_path) and not isFolder) then
				table.insert(diagnostics, {
					code = "class-on-non-folder",
					severity = "error",
					subject = mapping.configured_path,
					message = string.format(
						"%s sets $className to `%s` but $path is not a Folder",
						mapping.instance_path,
						mapping.class_name or ""
					),
					help = "remove `$className` or point `$path` at a directory without an init script",
				})
			end
		end
	end
	return diagnostics
end

-- 6. child-collision
local function checkChildCollision(_project: LoadedProject, mappings: { MappedPath }): { Diagnostic }
	local diagnostics: { Diagnostic } = {}
	for _, mapping in ipairs(mappings) do
		if
			#mapping.child_names > 0
			and fs.isDir(mapping.resolved_path)
			and not hasNestedProject(mapping.resolved_path)
		then
			local childLookup = {}
			for _, name in ipairs(mapping.child_names) do
				childLookup[name] = true
			end

			for _, entry in ipairs(fs.readDir(mapping.resolved_path)) do
				local full = Project.joinPaths(mapping.resolved_path, entry)
				local instanceName: string? = nil
				if fs.isDir(full) then
					if dirSnapshotIsFolder(full) then
						instanceName = entry
					end
				elseif fs.isFile(full) then
					instanceName = Project.instanceNameFromEntry(entry)
				end

				if instanceName and childLookup[instanceName] then
					table.insert(diagnostics, {
						code = "child-collision",
						severity = "error",
						subject = mapping.configured_path,
						message = string.format(
							"%s lists child `%s` which $path already provides from the filesystem",
							mapping.instance_path,
							instanceName
						),
						help = "remove the duplicate child from the project file or rename the filesystem entry",
					})
				end
			end
		end
	end
	return diagnostics
end

-- 7. ambiguous-init
local function checkAmbiguousInit(project: LoadedProject, mappings: { MappedPath }): { Diagnostic }
	local diagnostics: { Diagnostic } = {}

	local function walkDir(dir: string)
		if hasNestedProject(dir) then
			return
		end
		local inits: { string } = {}
		for _, entry in ipairs(fs.readDir(dir)) do
			local full = Project.joinPaths(dir, entry)
			if fs.isDir(full) then
				walkDir(full)
			elseif fs.isFile(full) then
				for _, initName in ipairs(INIT_SCRIPT_NAMES) do
					if entry == initName then
						table.insert(inits, entry)
						break
					end
				end
			end
		end

		if #inits >= 2 then
			table.sort(inits, function(a, b)
				local posA = table.find(INIT_SCRIPT_NAMES, a) or 999
				local posB = table.find(INIT_SCRIPT_NAMES, b) or 999
				return posA < posB
			end)
			table.insert(diagnostics, {
				code = "ambiguous-init",
				severity = "warning",
				subject = relativeSubject(project, dir),
				message = string.format(
					"contains multiple init scripts (%s); Rojo will only use the highest-priority one",
					table.concat(inits, ", ")
				),
				help = "keep a single init script (init.luau beats init.lua, then init.server.*, then init.client.*, then init.plugin.*, then init.csv)",
			})
		end
	end

	for _, mapping in ipairs(mappings) do
		if fs.isDir(mapping.resolved_path) then
			walkDir(mapping.resolved_path)
		end
	end
	return diagnostics
end

-- 8. orphan-meta
local function checkOrphanMeta(project: LoadedProject, mappings: { MappedPath }): { Diagnostic }
	local diagnostics: { Diagnostic } = {}

	local function walkDir(dir: string)
		if hasNestedProject(dir) then
			return
		end
		local entries = fs.readDir(dir)
		local metaStems: { [string]: string } = {}

		for _, entry in ipairs(entries) do
			local full = Project.joinPaths(dir, entry)
			if fs.isDir(full) then
				walkDir(full)
			elseif fs.isFile(full) then
				if entry:sub(-10) == ".meta.json" then
					local stem = entry:sub(1, -11)
					if stem ~= "init" and stem ~= "" then
						metaStems[stem] = entry
					end
				elseif entry:sub(-11) == ".meta.jsonc" then
					local stem = entry:sub(1, -12)
					if stem ~= "init" and stem ~= "" then
						metaStems[stem] = entry
					end
				end
			end
		end

		for stem, metaFile in pairs(metaStems) do
			local matched = false
			for _, sibling in ipairs(entries) do
				local sibFull = Project.joinPaths(dir, sibling)
				if
					(fs.isDir(sibFull) and sibling == stem)
					or (fs.isFile(sibFull) and Project.instanceNameFromEntry(sibling) == stem)
				then
					matched = true
					break
				end
			end
			if not matched then
				table.insert(diagnostics, {
					code = "orphan-meta",
					severity = "warning",
					subject = relativeSubject(project, Project.joinPaths(dir, metaFile)),
					message = "metadata file has no matching instance",
					help = string.format("add a sibling `%s` file or folder, or delete this .meta.json", stem),
				})
			end
		end
	end

	for _, mapping in ipairs(mappings) do
		if fs.isDir(mapping.resolved_path) then
			walkDir(mapping.resolved_path)
		end
	end
	return diagnostics
end

-- 9. script-context
local function checkScriptContext(project: LoadedProject, mappings: { MappedPath }): { Diagnostic }
	local diagnostics: { Diagnostic } = {}

	local function checkFile(filePath: string, instancePath: string, expected: "ServerOnly" | "ClientOnly")
		local fileName = filePath:match("[^/]+$") or filePath
		local isClient = fileName:sub(-12) == ".client.luau" or fileName:sub(-11) == ".client.lua"
		local isServer = fileName:sub(-12) == ".server.luau" or fileName:sub(-11) == ".server.lua"

		if expected == "ServerOnly" and isClient then
			table.insert(diagnostics, {
				code = "script-context",
				severity = "warning",
				subject = relativeSubject(project, filePath),
				message = string.format("client script placed under `%s` will not execute on the client", instancePath),
				help = "move this script to a client container (e.g. StarterPlayerScripts, StarterGui) or change to a server script",
			})
		elseif expected == "ClientOnly" and isServer then
			table.insert(diagnostics, {
				code = "script-context",
				severity = "warning",
				subject = relativeSubject(project, filePath),
				message = string.format("server script placed under `%s` will not execute on the client", instancePath),
				help = "move this script to ServerScriptService or change to a client script",
			})
		end
	end

	local function walkDir(dir: string, instancePath: string, expected: "ServerOnly" | "ClientOnly")
		if hasNestedProject(dir) then
			return
		end
		for _, entry in ipairs(fs.readDir(dir)) do
			local full = Project.joinPaths(dir, entry)
			if fs.isDir(full) then
				walkDir(full, instancePath, expected)
			elseif fs.isFile(full) then
				checkFile(full, instancePath, expected)
			end
		end
	end

	for _, mapping in ipairs(mappings) do
		local segs = mapping.instance_segments
		local expected: ("ServerOnly" | "ClientOnly")? = nil
		if segs[1] == "ServerScriptService" or segs[1] == "ServerStorage" then
			expected = "ServerOnly"
		elseif
			segs[1] == "ReplicatedFirst"
			or segs[1] == "StarterGui"
			or (segs[1] == "StarterPlayer" and segs[2] == "StarterPlayerScripts")
		then
			expected = "ClientOnly"
		end

		if expected then
			if fs.isFile(mapping.resolved_path) then
				checkFile(mapping.resolved_path, mapping.instance_path, expected)
			elseif fs.isDir(mapping.resolved_path) then
				walkDir(mapping.resolved_path, mapping.instance_path, expected)
			end
		end
	end

	return diagnostics
end

function Checks.runAll(project: LoadedProject): ({ Diagnostic }, number)
	local mappings = Project.collectMappedPaths(project)
	local diagnostics, resolvedPaths = checkMissingPaths(project, mappings)

	for _, d in ipairs(checkFileWithChildren(project, mappings)) do
		table.insert(diagnostics, d)
	end
	for _, d in ipairs(checkMissingName(project)) do
		table.insert(diagnostics, d)
	end
	for _, d in ipairs(checkMissingClass(project)) do
		table.insert(diagnostics, d)
	end
	for _, d in ipairs(checkClassOnNonFolder(project, mappings)) do
		table.insert(diagnostics, d)
	end
	for _, d in ipairs(checkChildCollision(project, mappings)) do
		table.insert(diagnostics, d)
	end
	for _, d in ipairs(checkAmbiguousInit(project, mappings)) do
		table.insert(diagnostics, d)
	end
	for _, d in ipairs(checkOrphanMeta(project, mappings)) do
		table.insert(diagnostics, d)
	end
	for _, d in ipairs(checkScriptContext(project, mappings)) do
		table.insert(diagnostics, d)
	end

	return diagnostics, resolvedPaths
end

function Checks.fixAll(project: LoadedProject): number
	local mappings = Project.collectMappedPaths(project)
	local orphanDiags = checkOrphanMeta(project, mappings)
	local fixedCount = 0

	for _, d in ipairs(orphanDiags) do
		local fullPath = Project.joinPaths(project.root_dir, d.subject)
		if fs.isFile(fullPath) then
			local ok = pcall(function()
				fs.removeFile(fullPath)
			end)
			if ok then
				fixedCount += 1
			end
		end
	end

	return fixedCount
end

return Checks
