export const defaultVersion = 'v0.3.1';

export const versionDefinitions = [
  {
    id: 'v0.3.1',
    release: '0.3.1',
    capabilities: ['atomic-update-claims', 'read-only-schema-admission'],
  },
  {
    id: 'v0.3.0',
    release: '0.3.0',
    capabilities: ['atomic-update-claims'],
  },
  {
    id: 'v0.2.0',
    release: '0.2.0',
    capabilities: [],
  },
];

export const versions = versionDefinitions.map(({ id }) => id);
