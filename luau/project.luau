--!strict
local fs = require("@lune/fs")
local serde = require("@lune/serde")
local Types = require("./types")

type LoadedProject = Types.LoadedProject
type ProjectNode = Types.ProjectNode
type MappedPath = Types.MappedPath

local Project = {}

local DEFAULT_PROJECT_FILES = { "default.project.json", "default.project.jsonc" }

local ROJO_SUFFIXES = {
	".server.luau",
	".server.lua",
	".client.luau",
	".client.lua",
	".plugin.luau",
	".plugin.lua",
	".project.jsonc",
	".project.json",
	".model.jsonc",
	".model.json",
	".luau",
	".lua",
	".jsonc",
	".json",
	".toml",
	".csv",
	".txt",
	".rbxmx",
	".rbxm",
	".yaml",
	".yml",
}

function Project.stripJsonc(input: string): string
	local result = {}
	local inString = false
	local inLineComment = false
	local inBlockComment = false
	local i = 1
	local len = #input

	while i <= len do
		local c = input:sub(i, i)
		local nextC = input:sub(i + 1, i + 1)

		if inLineComment then
			if c == "\n" then
				inLineComment = false
				table.insert(result, c)
			end
		elseif inBlockComment then
			if c == "*" and nextC == "/" then
				inBlockComment = false
				i += 1
			end
		elseif inString then
			table.insert(result, c)
			if c == "\\" then
				i += 1
				table.insert(result, input:sub(i, i))
			elseif c == '"' then
				inString = false
			end
		else
			if c == "/" and nextC == "/" then
				inLineComment = true
				i += 1
			elseif c == "/" and nextC == "*" then
				inBlockComment = true
				i += 1
			elseif c == '"' then
				inString = true
				table.insert(result, c)
			else
				table.insert(result, c)
			end
		end
		i += 1
	end

	local text = table.concat(result)
	text = text:gsub(",%s*([%}%]])", "%1")
	return text
end

function Project.isProjectFile(path: string): boolean
	return path:sub(-13) == ".project.json" or path:sub(-14) == ".project.jsonc"
end

function Project.normalizePath(path: string): string
	local clean = path:gsub("\\", "/")
	while clean:sub(-1) == "/" and #clean > 1 do
		clean = clean:sub(1, -2)
	end
	return clean
end

function Project.getDirectory(filePath: string): string
	local clean = Project.normalizePath(filePath)
	local lastSlash = clean:match("^(.*)/[^/]*$")
	return lastSlash or "."
end

function Project.joinPaths(a: string, b: string): string
	local normA = Project.normalizePath(a)
	local normB = Project.normalizePath(b)
	if normB:sub(1, 1) == "/" then
		return normB
	end
	if normA == "." or normA == "" then
		return normB
	end
	return normA .. "/" .. normB
end

function Project.discoverProjectFile(path: string): string
	local clean = Project.normalizePath(path)
	if fs.isFile(clean) then
		if Project.isProjectFile(clean) then
			return clean
		end
		error("path is not a .project.json or .project.jsonc file: " .. clean)
	elseif fs.isDir(clean) then
		for _, file in ipairs(DEFAULT_PROJECT_FILES) do
			local candidate = Project.joinPaths(clean, file)
			if fs.isFile(candidate) then
				return candidate
			end
		end
		error("no default.project.json or default.project.jsonc found in: " .. clean)
	else
		error("path does not exist: " .. clean)
	end
end

function Project.discoverProjectFiles(directory: string): { string }
	local dir = Project.normalizePath(directory)
	if not fs.isDir(dir) then
		error("not a directory: " .. dir)
	end
	local files = {}
	for _, entry in ipairs(fs.readDir(dir)) do
		if Project.isProjectFile(entry) then
			table.insert(files, Project.joinPaths(dir, entry))
		end
	end
	table.sort(files)
	return files
end

local function parseNode(raw: { [string]: any }): ProjectNode
	local className = raw["$className"]
	local rawPath = raw["$path"]
	local path: string? = nil
	local optionalPath: string? = nil

	if rawPath ~= nil then
		if type(rawPath) == "string" then
			path = rawPath
		elseif type(rawPath) == "table" and type(rawPath.optional) == "string" then
			optionalPath = rawPath.optional
		else
			error("Rojo project has an unsupported structure: invalid $path (expected string or { optional = string })")
		end
	end

	local children: { [string]: ProjectNode } = {}
	for k, v in pairs(raw) do
		if k:sub(1, 1) ~= "$" and type(v) == "table" then
			children[k] = parseNode(v)
		end
	end

	return {
		class_name = className,
		path = path,
		optional_path = optionalPath,
		children = children,
	}
end

function Project.loadProject(inputPath: string?): LoadedProject
	local resolvedFile = Project.discoverProjectFile(inputPath or ".")
	local rawContent = fs.readFile(resolvedFile)
	local stripped = Project.stripJsonc(rawContent)
	local parsed = serde.decode("json", stripped)

	local rootDir = Project.getDirectory(resolvedFile)
	local tree = parseNode(parsed.tree or {})

	return {
		path = resolvedFile,
		root_dir = rootDir,
		name = parsed.name,
		tree = tree,
	}
end

local function collectNodeMappings(
	node: ProjectNode,
	projectRoot: string,
	instancePath: { string },
	mappings: { MappedPath }
)
	local configuredPath = node.path or node.optional_path
	if configuredPath then
		local resolvedPath = Project.joinPaths(projectRoot, configuredPath)
		local childNames: { string } = {}
		for childName in pairs(node.children) do
			table.insert(childNames, childName)
		end
		table.sort(childNames)

		local instancePathStr = #instancePath == 0 and "project root" or table.concat(instancePath, ".")

		local segmentsCopy = {}
		for _, s in ipairs(instancePath) do
			table.insert(segmentsCopy, s)
		end

		table.insert(mappings, {
			instance_path = instancePathStr,
			instance_segments = segmentsCopy,
			configured_path = configuredPath,
			resolved_path = resolvedPath,
			optional = node.optional_path ~= nil,
			has_children = #childNames > 0,
			class_name = node.class_name,
			child_names = childNames,
		})
	end

	local sortedChildren = {}
	for k in pairs(node.children) do
		table.insert(sortedChildren, k)
	end
	table.sort(sortedChildren)

	for _, childName in ipairs(sortedChildren) do
		table.insert(instancePath, childName)
		collectNodeMappings(node.children[childName], projectRoot, instancePath, mappings)
		table.remove(instancePath)
	end
end

function Project.collectMappedPaths(project: LoadedProject): { MappedPath }
	local mappings: { MappedPath } = {}
	local instancePath: { string } = {}
	collectNodeMappings(project.tree, project.root_dir, instancePath, mappings)
	return mappings
end

function Project.instanceNameFromEntry(fileName: string): string?
	for _, suffix in ipairs(ROJO_SUFFIXES) do
		if fileName:sub(-#suffix) == suffix then
			local stem = fileName:sub(1, -#suffix - 1)
			if stem == "init" or stem == "" then
				return nil
			end
			return stem
		end
	end
	return nil
end

return Project
