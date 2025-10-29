import axios from 'axios';
import { spawn, spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { Agent } from 'node:https';
import * as semver from 'semver';

const httpClient = axios.create({
  baseURL: 'https://localhost:7443',
  withCredentials: true,
  httpsAgent: new Agent({
    keepAlive: false,
    rejectUnauthorized: false,
  }),
});

httpClient.interceptors.response.use((response) => {
  if (response.headers['set-cookie']?.[0]) {
    //@ts-expect-error forced
    httpClient.sessionToken = response.headers['set-cookie']?.[0].split(';')[0];
  }
  return response;
});

httpClient.interceptors.request.use(
  (config) => {
    //@ts-expect-error forced
    config.headers['Cookie'] = httpClient.sessionToken;
    return config;
  },
  (error) => Promise.reject(error),
);

const setupAPI7 = async () => {
  return new Promise<void>((resolve, reject) => {
    const download = spawnSync(
      'sh',
      [
        '-c',
        `curl -O ${process.env.BACKEND_API7_DOWNLOAD_URL} && tar xf api7-ee-*.tar.gz`,
      ],
      { cwd: `/tmp` },
    );

    console.log('stdout: ' + download.stdout.toString('utf-8'));
    console.log('stderr: ' + download.stderr.toString('utf-8'));

    const dockerComposePath = `/tmp/api7-ee/docker-compose.yaml`;
    const dockerCompose = readFileSync(dockerComposePath, 'utf-8').replaceAll(
      ': bitnami/',
      ': bitnamilegacy/',
    );
    writeFileSync(dockerComposePath, dockerCompose, 'utf-8');

    const setup = spawn('sh', ['-c', `cd api7-ee && bash run.sh start`], {
      cwd: `/tmp`,
    });

    console.log('\nSetup API7 Instance\n');

    setup.stdout.on('data', function (data) {
      console.log('stdout: ' + data.toString());
    });

    setup.stderr.on('data', function (data) {
      console.log('stderr: ' + data.toString());
    });

    setup.on('exit', function (code) {
      if (code)
        reject(`child process exited with non-zero code: ${code?.toString()}`);

      console.log('Successful deployment');
      resolve();
    });
  });
};

const initUser = async (
  username = 'admin',
  password = 'admin',
  fisrtTime = true,
) => {
  console.log('Log in');
  await httpClient.post(`/api/login`, {
    username: username,
    password: password,
  });

  // If the version is lower than 3.2.15, the license should be activated first.
  if (
    semver.lt(
      semver.coerce(process.env.BACKEND_API7_VERSION) ?? '0.0.0',
      '3.2.15',
    ) &&
    fisrtTime
  )
    await activateAPI7();

  console.log('Modify password');
  await httpClient.put(`/api/password`, {
    old_password: password,
    new_password: 'Admin12345!',
  });

  //@ts-expect-error forced
  httpClient.sessionToken = '';

  console.log('Log in again');
  await httpClient.post(`/api/login`, {
    username: username,
    password: 'Admin12345!',
  });

  // If the version is greater than or equal to 3.2.15, the license should
  // be activated after changing the password.
  if (
    semver.gte(
      semver.coerce(process.env.BACKEND_API7_VERSION) ?? '0.0.0',
      '3.2.15',
    ) &&
    fisrtTime
  )
    await activateAPI7();
};

const activateAPI7 = async () => {
  console.log('Upload license');
  await httpClient.put(`/api/license`, {
    data: process.env.BACKEND_API7_LICENSE,
  });
};

const generateToken = async () => {
  console.log('Create test user');
  const user = await httpClient.post(`/api/invites`, {
    username: 'test',
    password: 'test',
  });
  const userId: string = user.data.value.id;

  console.log('Update role');
  await httpClient.put(`/api/users/${userId}/assigned_roles`, {
    roles: ['super_admin_id'],
  });

  //@ts-expect-error forced
  httpClient.sessionToken = '';

  console.log('Log in to test user');
  await initUser('test', 'test', false);

  console.log('Generate token');
  const resp = await httpClient.post<{ value: { token: string } }>(
    `/api/tokens`,
    {
      expires_at: 0,
      name: randomUUID(),
    },
  );

  process.env.TOKEN = resp.data.value.token;
};

export default async () => {
  if (process.env['SKIP_API7_SETUP'] !== 'true') await setupAPI7();
  try {
    await initUser();
    await generateToken();
  } catch (err) {
    console.log(err);
    throw err;
  }

  process.env.SERVER = 'https://localhost:7443';
  process.env.GATEWAY_GROUP = 'default';

  // ONLY FOR LOCAL TEST //
  /* process.env.SERVER = 'https://localhost:7443';
  process.env.TOKEN = 'a7ee-token';
  process.env.GATEWAY_GROUP = 'adc';
  process.env.BACKEND_API7_VERSION = '0.0.0'; */
  // ONLY FOR LOCAL TEST //
};
