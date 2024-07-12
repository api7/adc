import axios from 'axios';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { Agent } from 'node:https';

const httpClient = axios.create({
  baseURL: 'https://localhost:7443',
  auth: {
    username: 'admin',
    password: 'admin',
  },
  httpsAgent: new Agent({
    rejectUnauthorized: false,
  }),
});

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

const activateAPI7 = async () => {
  await httpClient.put(`/api/license`, {
    data: process.env.BACKEND_API7_LICENSE,
  });
};

const generateToken = async () => {
  const resp = await httpClient.post<{ value: { token: string } }>(
    `/api/tokens`,
    {
      expires_at: 0,
      name: randomUUID(),
    },
    { validateStatus: () => true },
  );

  globalThis.token = resp.data.value.token;
};

export default async () => {
  if (process.env['SKIP_API7_SETUP'] !== 'true') await setupAPI7();
  await activateAPI7();
  await generateToken();

  globalThis.server = 'https://localhost:7443';
};
