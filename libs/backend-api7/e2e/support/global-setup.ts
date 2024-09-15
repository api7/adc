import axios from 'axios';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { Agent } from 'node:https';

const httpClient = axios.create({
  baseURL: 'https://localhost:7443',
  withCredentials: true,
  httpsAgent: new Agent({
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
    const ls = spawn(
      'sh',
      [
        '-c',
        `curl -O ${process.env.BACKEND_API7_DOWNLOAD_URL} && tar xf api7-ee-*.tar.gz && cd api7-ee && bash run.sh start`,
      ],
      { cwd: `/tmp` },
    );

    console.log('\nSetup API7 Instance\n');

    ls.stdout.on('data', function (data) {
      console.log('stdout: ' + data.toString());
    });

    ls.stderr.on('data', function (data) {
      console.log('stderr: ' + data.toString());
    });

    ls.on('exit', function (code) {
      if (code)
        reject(`child process exited with non-zero code: ${code?.toString()}`);

      console.log('Successful deployment');
      resolve();
    });
  });
};

const initUser = async (username = 'admin', password = 'admin') => {
  console.log('Log in');
  await httpClient.post(`/api/login`, {
    username: username,
    password: password,
  });

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
  await initUser('test', 'test');

  console.log('Generate token');
  const resp = await httpClient.post<{ value: { token: string } }>(
    `/api/tokens`,
    {
      expires_at: 0,
      name: randomUUID(),
    },
    { validateStatus: () => true },
  );

  process.env.TOKEN = resp.data.value.token;
};

export default async () => {
  if (process.env['SKIP_API7_SETUP'] !== 'true') await setupAPI7();
  await initUser();
  await activateAPI7();
  await generateToken();

  process.env.SERVER = 'https://localhost:7443';
};
