"""Parse dependency declarations and rewrite only their fork revision values."""

import json

try:
    import tomllib
except ImportError:
    # Xcode's stock Python 3.9 ships this TOML 1.0 parser with pip.
    from pip._vendor import tomli as tomllib

FORK_URL = "https://github.com/shin1ohno/herdr-gpui"
DEPENDENCIES = {"dependencies", "dev-dependencies", "build-dependencies"}


def dependency_locations(document):
    """Cargo dependency scopes, excluding unrelated metadata tables."""
    for key in DEPENDENCIES:
        if key in document:
            yield (key,), document[key]
    workspace = document.get("workspace", {})
    if "dependencies" in workspace:
        yield ("workspace", "dependencies"), workspace["dependencies"]
    for target, settings in document.get("target", {}).items():
        for key in DEPENDENCIES:
            if key in settings:
                yield ("target", target, key), settings[key]


def dependency_tables(document):
    for _, table in dependency_locations(document):
        yield table


def fork_dependencies(text):
    return [value for table in dependency_tables(tomllib.loads(text))
            for value in table.values()
            if isinstance(value, dict) and value.get("git") == FORK_URL]


def only_path(document):
    path = []
    while isinstance(document, dict) and len(document) == 1:
        key, document = next(iter(document.items()))
        path.append(key)
    return tuple(path)


def value_text(value):
    if isinstance(value, dict):
        return "{ " + ", ".join(json.dumps(k) + " = " + value_text(v)
                                for k, v in value.items()) + " }"
    if isinstance(value, list):
        return "[" + ", ".join(value_text(v) for v in value) + "]"
    return json.dumps(value, ensure_ascii=False)


def update_manifest(text, revision):
    document = tomllib.loads(text)
    forks = fork_dependencies(text)
    locations = {path for path, _ in dependency_locations(document)}
    if any("rev" not in dep or "branch" in dep or "tag" in dep for dep in forks):
        raise ValueError("every fork dependency must use a pinned rev without branch/tag")
    # Parse complete TOML statements, including multiline inline tables/strings.
    # Using the parser for section/key paths also handles quoted target names.
    section = ()
    pending = ""
    output = []
    for line in text.splitlines(keepends=True):
        pending += line
        try:
            statement = tomllib.loads(pending)
        except tomllib.TOMLDecodeError:
            continue
        if not statement:
            output.append(pending)
            pending = ""
            continue
        if pending.lstrip().startswith("["):
            section = only_path(statement)
            output.append(pending)
        else:
            # A declaration's rev can live inside an inline dependency or a
            # dedicated dependency table. Resolve it against the parsed tree.
            target = document
            for key in section:
                target = target[key]
            changed = False
            for key, value in statement.items():
                if (section in locations
                        and isinstance(value, dict) and value.get("git") == FORK_URL):
                    value["rev"] = revision
                    output.append(json.dumps(key) + " = " + value_text(value) + "\n")
                    changed = True
                elif (section[:-1] in locations
                      and target.get("git") == FORK_URL and key == "rev"):
                    output.append("rev = " + json.dumps(revision) + "\n")
                    changed = True
            if not changed:
                output.append(pending)
        pending = ""
    if pending:
        raise ValueError("incomplete TOML statement")
    result = "".join(output)
    updated = fork_dependencies(result)
    if len(updated) != len(forks) or any(dep.get("rev") != revision for dep in updated):
        raise ValueError("not every fork dependency was updated to the requested revision")
    # Apart from rev fields, the parsed manifest must remain exactly the same.
    expected = tomllib.loads(text)
    for table in dependency_tables(expected):
        for dependency in table.values():
            if isinstance(dependency, dict) and dependency.get("git") == FORK_URL:
                dependency["rev"] = revision
    if tomllib.loads(result) != expected:
        raise ValueError("fork bump changed unrelated manifest values")
    return result, len(forks)
