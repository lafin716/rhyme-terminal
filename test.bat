@echo off
chcp 65001 >nul
setlocal enabledelayedexpansion

echo ===========================================
echo  SQS 큐 일괄 삭제 스크립트
echo  총 115개 큐를 삭제합니다.
echo ===========================================
echo.
set /p CONFIRM=정말로 삭제를 진행하시겠습니까? (Y/N): 
if /I not "%CONFIRM%"=="Y" (
    echo 취소되었습니다.
    pause
    exit /b
)

set /a COUNT=0

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-item-update-cart-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-item-update-cart-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-item-update-event-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-item-update-event-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-update-cart-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-update-cart-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-update-event-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-update-event-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-cancel-event-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-cancel-event-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-cancel-notification-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-cancel-notification-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-cancel-publish-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-cancel-publish-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-receive-event-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-receive-event-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-receive-publish-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-receive-publish-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-product-update-cart-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-product-update-cart-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-product-update-event-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-product-update-event-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-delete-cart-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-delete-cart-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-delete-event-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-delete-event-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-update-cart-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-update-cart-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-update-event-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-update-event-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-create-cart-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-create-cart-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-create-event-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-create-event-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-delete-cart-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-delete-cart-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-delete-event-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-delete-event-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-update-cart-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-update-cart-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-update-event-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-update-event-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-store-close-cart-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-store-close-cart-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-store-close-event-storebatch
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-store-close-event-storebatch"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-item-update-cart-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-item-update-cart-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-item-update-cart-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-item-update-cart-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-item-update-event-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-item-update-event-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-item-update-event-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-item-update-event-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-update-cart-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-update-cart-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-update-cart-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-update-cart-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-update-event-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-update-event-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-update-event-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-update-event-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-cancel-event-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-cancel-event-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-cancel-event-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-cancel-event-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-cancel-notification-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-cancel-notification-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-cancel-notification-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-cancel-notification-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-cancel-publish-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-cancel-publish-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-cancel-publish-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-cancel-publish-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-receive-event-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-receive-event-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-receive-event-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-receive-event-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-receive-publish-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-receive-publish-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-receive-publish-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-receive-publish-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-product-update-cart-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-product-update-cart-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-product-update-cart-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-product-update-cart-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-product-update-event-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-product-update-event-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-product-update-event-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-product-update-event-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-delete-cart-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-delete-cart-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-delete-cart-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-delete-cart-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-delete-event-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-delete-event-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-delete-event-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-delete-event-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-update-cart-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-update-cart-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-update-cart-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-update-cart-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-update-event-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-update-event-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-update-event-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-update-event-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-create-cart-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-create-cart-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-create-cart-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-create-cart-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-create-event-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-create-event-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-create-event-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-create-event-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-delete-cart-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-delete-cart-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-delete-cart-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-delete-cart-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-delete-event-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-delete-event-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-delete-event-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-delete-event-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-update-cart-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-update-cart-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-update-cart-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-update-cart-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-update-event-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-update-event-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-update-event-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-update-event-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-store-close-cart-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-store-close-cart-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-store-close-cart-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-store-close-cart-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-store-close-event-productorderinfo
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-store-close-event-productorderinfo"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-store-close-event-productorderinfos
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-store-close-event-productorderinfos"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-item-update-cart-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-item-update-cart-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-item-update-event-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-item-update-event-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-update-cart-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-update-cart-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-update-event-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-update-event-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-cancel-event-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-cancel-event-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-cancel-notification-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-cancel-notification-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-cancel-publish-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-cancel-publish-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-receive-event-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-receive-event-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-receive-publish-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-receive-publish-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-product-update-cart-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-product-update-cart-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-product-update-event-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-product-update-event-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-delete-cart-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-delete-cart-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-delete-event-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-delete-event-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-update-cart-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-update-cart-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-update-event-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-update-event-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-create-cart-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-create-cart-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-create-event-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-create-event-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-delete-cart-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-delete-cart-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-delete-event-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-delete-event-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-update-cart-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-update-cart-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-update-event-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-update-event-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-store-close-cart-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-store-close-cart-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-store-close-event-adsadsad
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-store-close-event-adsadsad"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-item-update-cart-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-item-update-cart-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-item-update-event-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-item-update-event-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-update-cart-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-update-cart-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-option-group-update-event-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-option-group-update-event-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-cancel-event-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-cancel-event-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-cancel-notification-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-cancel-notification-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-cancel-publish-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-cancel-publish-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-receive-event-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-receive-event-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-order-receive-publish-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-order-receive-publish-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-product-update-cart-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-product-update-cart-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-product-update-event-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-product-update-event-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-delete-cart-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-delete-cart-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-delete-event-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-delete-event-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-update-cart-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-update-cart-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-category-update-event-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-category-update-event-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-create-cart-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-create-cart-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-create-event-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-create-event-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-delete-cart-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-delete-cart-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-delete-event-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-delete-event-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-update-cart-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-update-cart-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-sale-item-update-event-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-sale-item-update-event-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-store-close-cart-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-store-close-cart-menu"

set /a COUNT+=1
echo [!COUNT!/115] 삭제 중: easyqr-store-close-event-menu
call aws sqs delete-queue --queue-url "https://sqs.ap-northeast-2.amazonaws.com/339713105449/easyqr-store-close-event-menu"

echo ===========================================
echo  삭제 완료 (%COUNT%개 처리)
echo ===========================================
pause