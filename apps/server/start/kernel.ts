import router from '@c9up/ream/services/router'

router.use([() => import('@c9up/ream/bodyparser_middleware'), () => import('@c9up/blackhole/middleware')])
