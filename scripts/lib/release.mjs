const NUMERIC_IDENTIFIER_PATTERN = "(?:0|[1-9][0-9]*)";
const PRERELEASE_IDENTIFIER_PATTERN =
  "(?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*)";
const PRERELEASE_PATTERN = `(?:-${PRERELEASE_IDENTIFIER_PATTERN}(?:\\.${PRERELEASE_IDENTIFIER_PATTERN})*)?`;
const RELEASE_TAG_PATTERN = new RegExp(
  `^v${NUMERIC_IDENTIFIER_PATTERN}\\.${NUMERIC_IDENTIFIER_PATTERN}\\.${NUMERIC_IDENTIFIER_PATTERN}${PRERELEASE_PATTERN}$`,
);
const RELEASE_VERSION_PATTERN = new RegExp(
  `^(?<major>${NUMERIC_IDENTIFIER_PATTERN})\\.(?<minor>${NUMERIC_IDENTIFIER_PATTERN})\\.(?<patch>${NUMERIC_IDENTIFIER_PATTERN})(?:-(?<prerelease>${PRERELEASE_IDENTIFIER_PATTERN}(?:\\.${PRERELEASE_IDENTIFIER_PATTERN})*))?$`,
);

export function resolveReleasePlan(tag) {
  if (!RELEASE_TAG_PATTERN.test(tag)) {
    throw new Error("Tag must look like v1.2.3 or v1.2.3-beta.1");
  }

  const version = tag.slice(1);
  const isPrerelease = version.includes("-");

  return {
    tag,
    version,
    isPrerelease,
    npmDistTag: isPrerelease ? "next" : "latest",
  };
}

function parseReleaseVersion(version) {
  const match = RELEASE_VERSION_PATTERN.exec(version);

  if (!match?.groups) {
    throw new Error(`Version must look like 1.2.3 or 1.2.3-beta.1: ${version}`);
  }

  return {
    major: Number(match.groups.major),
    minor: Number(match.groups.minor),
    patch: Number(match.groups.patch),
    prerelease: match.groups.prerelease ? match.groups.prerelease.split(".") : [],
  };
}

function comparePrereleaseIdentifiers(left, right) {
  const leftIsNumeric = /^[0-9]+$/.test(left);
  const rightIsNumeric = /^[0-9]+$/.test(right);

  if (leftIsNumeric && rightIsNumeric) {
    return Number(left) === Number(right) ? 0 : Number(left) < Number(right) ? -1 : 1;
  }

  if (leftIsNumeric !== rightIsNumeric) {
    return leftIsNumeric ? -1 : 1;
  }

  if (left === right) {
    return 0;
  }

  return left < right ? -1 : 1;
}

export function compareReleaseVersions(leftVersion, rightVersion) {
  const left = parseReleaseVersion(leftVersion);
  const right = parseReleaseVersion(rightVersion);

  for (const key of ["major", "minor", "patch"]) {
    if (left[key] !== right[key]) {
      return left[key] < right[key] ? -1 : 1;
    }
  }

  if (left.prerelease.length === 0 && right.prerelease.length === 0) {
    return 0;
  }

  if (left.prerelease.length === 0) {
    return 1;
  }

  if (right.prerelease.length === 0) {
    return -1;
  }

  const length = Math.max(left.prerelease.length, right.prerelease.length);
  for (let index = 0; index < length; index += 1) {
    const leftIdentifier = left.prerelease[index];
    const rightIdentifier = right.prerelease[index];

    if (leftIdentifier === undefined) {
      return -1;
    }

    if (rightIdentifier === undefined) {
      return 1;
    }

    const comparison = comparePrereleaseIdentifiers(leftIdentifier, rightIdentifier);
    if (comparison !== 0) {
      return comparison;
    }
  }

  return 0;
}

export function assertReleaseUpgrade(currentVersion, nextVersion) {
  if (compareReleaseVersions(currentVersion, nextVersion) >= 0) {
    throw new Error(
      `Target version ${nextVersion} must be greater than current version ${currentVersion}`,
    );
  }
}
