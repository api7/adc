import { execSync } from 'child_process';
import { cpSync, existsSync, mkdirSync, rmSync, unlinkSync } from 'fs';
import { Listr } from 'listr2';

const version = process.env.NODE_VERSION ?? '20.15.1';

const tasks = new Listr([
  {
    title: 'Download NodeJS multi-arch binary',
    task: (_, task): Listr =>
      task.newListr([
        {
          title: 'Download Linux AMD64',
          task: async (): Promise<void> => {
            const file = 'linux-x64.tar.gz';
            if (!existsSync(`./${file}`))
              execSync(
                `wget -q -O ${file} https://nodejs.org/dist/v${version}/node-v${version}-linux-x64.tar.gz`,
              );
          },
        },
        {
          title: 'Download Linux ARM64',
          task: async (): Promise<void> => {
            const file = 'linux-arm64.tar.gz';
            if (!existsSync(`./${file}`))
              execSync(
                `wget -q -O ${file} https://nodejs.org/dist/v${version}/node-v${version}-linux-arm64.tar.gz`,
              );
          },
        },
        {
          title: 'Download Windows AMD64',
          task: async (): Promise<void> => {
            const file = 'win-x64.zip';
            if (!existsSync(`./${file}`))
              execSync(
                `wget -q -O ${file} https://nodejs.org/dist/v${version}/node-v${version}-win-x64.zip`,
              );
          },
        },
        {
          title: 'Download Windows ARM64',
          task: async (): Promise<void> => {
            const file = 'win-arm64.zip';
            if (!existsSync(`./${file}`))
              execSync(
                `wget -q -O ${file} https://nodejs.org/dist/v${version}/node-v${version}-win-arm64.zip`,
              );
          },
        },
        {
          title: 'Download macOS ARM64',
          task: async (): Promise<void> => {
            const file = 'darwin-arm64.tar.gz';
            if (!existsSync(`./${file}`))
              execSync(
                `wget -q -O ${file} https://nodejs.org/dist/v${version}/node-v${version}-darwin-arm64.tar.gz`,
              );
          },
        },
        {
          title: 'Download macOS AMD64',
          task: async (): Promise<void> => {
            const file = 'darwin-x64.tar.gz';
            if (!existsSync(`./${file}`))
              execSync(
                `wget -q -O ${file} https://nodejs.org/dist/v${version}/node-v${version}-darwin-x64.tar.gz`,
              );
          },
        },
      ]),
  },
  {
    title: 'Extract NodeJS multi-arch binary',
    task: () => {
      execSync('tar -xvzf linux-x64.tar.gz');
      execSync('tar -xvzf linux-arm64.tar.gz');
      execSync('unzip win-x64.zip');
      execSync('unzip win-arm64.zip');
      execSync('tar -xvzf darwin-arm64.tar.gz');
      execSync('tar -xvzf darwin-x64.tar.gz');
    },
  },
  {
    title: 'Copy all NodeJS binary',
    task: () => {
      mkdirSync('./node-binary');
      cpSync(
        `./node-v${version}-linux-x64/bin/node`,
        './node-binary/linux-amd64',
      );
      cpSync(
        `./node-v${version}-linux-arm64/bin/node`,
        './node-binary/linux-arm64',
      );
      cpSync(
        `./node-v${version}-win-x64/node.exe`,
        './node-binary/win-x64.exe',
      );
      cpSync(
        `./node-v${version}-win-arm64/node.exe`,
        './node-binary/win-arm64.exe',
      );
      cpSync(
        `./node-v${version}-darwin-arm64/bin/node`,
        './node-binary/darwin-arm64',
      );
      cpSync(
        `./node-v${version}-darwin-x64/bin/node`,
        './node-binary/darwin-x64',
      );
    },
  },
  {
    title: 'Clean temporary files',
    task: () => {
      const opts = { recursive: true, force: true };
      rmSync(`./node-v${version}-linux-x64`, opts);
      rmSync(`./node-v${version}-linux-arm64`, opts);
      rmSync(`./node-v${version}-win-x64`, opts);
      rmSync(`./node-v${version}-win-arm64`, opts);
      rmSync(`./node-v${version}-darwin-arm64`, opts);
      rmSync(`./node-v${version}-darwin-x64`, opts);
      unlinkSync('./linux-x64.tar.gz');
      unlinkSync('./linux-arm64.tar.gz');
      unlinkSync('./win-x64.zip');
      unlinkSync('./win-arm64.zip');
      unlinkSync('./darwin-arm64.tar.gz');
      unlinkSync('./darwin-x64.tar.gz');
    },
  },
]);

(async function () {
  try {
    await tasks.run();
  } catch (e) {
    console.error(e);
  }
})();
