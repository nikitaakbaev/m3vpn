# M0 spikes

Скрипты, которыми проверялись технические риски. Все результаты — в
[`docs/m0-report.md`](../../docs/m0-report.md).

## Подготовка

```bash
bash scripts/build-sidecars.sh
```

Скрипт скачивает локальный Go, собирает `hiddify-sing-box` (включая `with_awg`),
скачивает Xray и geo-ассеты. Секреты и бинарники в git не попадают.

## Подписка

```bash
# положить URL в .secrets/subscription.url (chmod 600) и скачать тело
python3 scripts/spikes/inspect_subscription.py
python3 scripts/spikes/inspect_config.py 4
```

`inspect_config.py` пишет санитизированный конфиг в `.secrets/xray-test.json`
и принудительно слушает только `127.0.0.1:20808/20809`.

## Полный Xray-конфиг

```bash
bash scripts/spikes/run_xray_test.sh 4
```

Нужен `XRAY_LOCATION_ASSET=sidecars/assets` (скрипт выставляет сам).

## VLESS Reality через sing-box

```bash
python3 scripts/spikes/xray_vless_to_singbox.py \
  --config-index 0 --outbound-index 2 -o .secrets/vless-singbox.json
sidecars/bin/hiddify-sing-box check -c .secrets/vless-singbox.json
sidecars/bin/hiddify-sing-box run -c .secrets/vless-singbox.json
curl --socks5-hostname 127.0.0.1:20808 https://api.ipify.org
```

## AmneziaWG

Нужен локальный сервер. Пример с Docker:

```bash
docker volume create m3awg
docker run --rm -v m3awg:/etc/amnezia/amneziawg --entrypoint sh metaligh/amneziawg -c '
  cd /etc/amnezia/amneziawg
  printf "127.0.0.1\n1.1.1.1, 8.8.8.8\n51830\n" > serverip.cfg
  ./awguser </dev/null
'
docker run -d --name m3awg --cap-add NET_ADMIN --device /dev/net/tun \
  -p 127.0.0.1:51830:51830/udp -v m3awg:/etc/amnezia/amneziawg metaligh/amneziawg
docker exec m3awg cat /etc/amnezia/amneziawg/metaligh.conf > .secrets/awg-client.conf
chmod 600 .secrets/awg-client.conf
python3 scripts/spikes/awg_conf_to_singbox.py \
  .secrets/awg-client.conf -o .secrets/awg-singbox.json
sidecars/bin/hiddify-sing-box run -c .secrets/awg-singbox.json
docker exec m3awg awg show
```

## Hot-switch

```bash
bash scripts/spikes/selector_switch_test.sh
```

## TUN restore

```bash
bash scripts/spikes/tun_restore_test.sh
```

Тест работает в `unshare -Urn` и не меняет сеть хоста. Если в системе запрещены
rootless user namespaces, тест нужно запускать иначе.
